use super::{
    now_rfc3339, validate_digest, BlobRecord, LayerRecord, ManifestRecord, RefConflictPolicy,
    RefRecord, RefUpdate, SQLITE_INDEX_FILE_NAME,
};
use anyhow::{ensure, Context, Result};
use ocipkg::ImageName;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::{
    fs,
    path::Path,
    sync::{Mutex, MutexGuard},
    time::Duration,
};

/// Public outcome of [`SqliteIndexStore::publish_artifact_atomic`].
///
/// `ref_update` is `None` when the caller did not pass an `image_name`
/// (digest-only publish, no SQLite ref written). Otherwise it carries
/// the [`RefUpdate`] for that ref.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishOutcome {
    pub ref_update: Option<RefUpdate>,
}

const SCHEMA_VERSION: i64 = 1;
const SQLITE_BUSY_TIMEOUT: Duration = Duration::from_secs(30);

/// SQLite-backed index store for the v3 Local Registry.
///
/// `rusqlite::Connection` is `Send` but `!Sync`, so it lives behind a
/// [`Mutex`] here. That makes [`SqliteIndexStore`] (and the enclosing
/// [`super::LocalRegistry`]) `Sync`, which lets a single registry be
/// shared via [`std::sync::Arc`] across PyO3 wrappers without
/// per-artifact connection duplication.
#[derive(Debug)]
pub struct SqliteIndexStore {
    conn: Mutex<Connection>,
}

impl SqliteIndexStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open SQLite index {}", path.display()))?;
        conn.busy_timeout(SQLITE_BUSY_TIMEOUT)
            .context("Failed to configure SQLite busy timeout")?;
        // Best-effort WAL: better concurrency for readers + writer,
        // but the PRAGMA itself needs an exclusive lock at the moment
        // of switching, which can fail with SQLITE_BUSY if another
        // process is opening the same file at the same instant. Once
        // *any* connection has set WAL on the file, every subsequent
        // opener inherits it, so a lost race here is harmless.
        // Correctness still comes from `BEGIN IMMEDIATE` +
        // `busy_timeout`, not from the journal mode.
        let _ = conn.pragma_update(None, "journal_mode", "WAL");
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_schema()?;
        Ok(store)
    }

    pub fn open_in_registry_root(root: impl AsRef<Path>) -> Result<Self> {
        Self::open(root.as_ref().join(SQLITE_INDEX_FILE_NAME))
    }

    /// Acquire the connection guard. If the mutex was poisoned by an
    /// earlier panic, fall back to the wrapped `Connection` (rusqlite's
    /// internal state isn't damaged by Rust-level poisoning — the lock
    /// flag just records that some other thread paniced while holding
    /// it) and log a warning. Better than letting one rogue panic
    /// abort every subsequent registry call, which matters for the
    /// Python bindings where the same process may run many independent
    /// operations.
    fn lock(&self) -> MutexGuard<'_, Connection> {
        match self.conn.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                tracing::warn!(
                    "SqliteIndexStore connection mutex was poisoned by an earlier panic; \
                     recovering the inner connection"
                );
                poisoned.into_inner()
            }
        }
    }

    pub fn schema_version(&self) -> Result<i64> {
        self.lock()
            .query_row(
                "SELECT version FROM ommx_local_registry_schema LIMIT 1",
                [],
                |row| row.get(0),
            )
            .context("Failed to read local registry schema version")
    }

    pub fn put_blob(&self, record: &BlobRecord) -> Result<()> {
        let conn = self.lock();
        Self::put_blob_in(&conn, record)
    }

    /// Connection-scoped variant of [`Self::put_blob`]. Lets callers
    /// compose blob inserts with manifest / ref inserts inside one
    /// transaction (see [`Self::publish_artifact_atomic`]). The
    /// existence-and-size guard is run inside the same `conn`/`tx`
    /// so the check + insert are read-after-write consistent.
    fn put_blob_in(conn: &Connection, record: &BlobRecord) -> Result<()> {
        validate_digest(&record.digest)?;
        if let Some(existing_size) = conn
            .query_row(
                "SELECT size FROM blobs WHERE digest = ?1",
                params![record.digest],
                |row| read_u64(row, 0),
            )
            .optional()
            .context("Failed to query existing blob size")?
        {
            ensure!(
                existing_size == record.size,
                "Blob size mismatch for digest {}: existing={}, new={}",
                record.digest,
                existing_size,
                record.size
            );
        }
        let now = now_rfc3339();
        conn.execute(
            r#"
            INSERT INTO blobs (digest, size, media_type, storage_uri, kind, created_at, last_verified_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(digest) DO UPDATE SET
                size = excluded.size,
                media_type = excluded.media_type,
                storage_uri = excluded.storage_uri,
                kind = excluded.kind,
                last_verified_at = excluded.last_verified_at
            "#,
            params![
                record.digest,
                i64::try_from(record.size).context("Blob size does not fit in i64")?,
                record.media_type,
                record.storage_uri,
                record.kind,
                now,
                record.last_verified_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_blob(&self, digest: &str) -> Result<Option<BlobRecord>> {
        validate_digest(digest)?;
        self.lock()
            .query_row(
                r#"
                SELECT digest, size, media_type, storage_uri, kind, last_verified_at
                FROM blobs
                WHERE digest = ?1
                "#,
                params![digest],
                |row| {
                    Ok(BlobRecord {
                        digest: row.get(0)?,
                        size: read_u64(row, 1)?,
                        media_type: row.get(2)?,
                        storage_uri: row.get(3)?,
                        kind: row.get(4)?,
                        last_verified_at: row.get(5)?,
                    })
                },
            )
            .optional()
            .context("Failed to query blob record")
    }

    pub fn put_manifest(&self, record: &ManifestRecord, layers: &[LayerRecord]) -> Result<()> {
        let mut conn = self.lock();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        Self::put_manifest_in(&tx, record, layers)?;
        tx.commit()?;
        Ok(())
    }

    /// Connection-scoped variant of [`Self::put_manifest`]. Caller is
    /// responsible for opening / committing the transaction so the
    /// manifest insert can be composed with other writes (blob
    /// records, ref publish) atomically.
    fn put_manifest_in(
        conn: &Connection,
        record: &ManifestRecord,
        layers: &[LayerRecord],
    ) -> Result<()> {
        validate_digest(&record.digest)?;
        conn.execute(
            r#"
            INSERT INTO manifests (digest, media_type, size, subject_digest, annotations_json, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(digest) DO UPDATE SET
                media_type = excluded.media_type,
                size = excluded.size,
                subject_digest = excluded.subject_digest,
                annotations_json = excluded.annotations_json
            "#,
            params![
                record.digest,
                record.media_type,
                i64::try_from(record.size).context("Manifest size does not fit in i64")?,
                record.subject_digest,
                record.annotations_json,
                record.created_at,
            ],
        )?;
        conn.execute(
            "DELETE FROM manifest_layers WHERE manifest_digest = ?1",
            params![record.digest],
        )?;
        for layer in layers {
            ensure!(
                layer.manifest_digest == record.digest,
                "Layer manifest digest mismatch: {} != {}",
                layer.manifest_digest,
                record.digest
            );
            validate_digest(&layer.digest)?;
            conn.execute(
                r#"
                INSERT INTO manifest_layers
                    (manifest_digest, position, digest, media_type, size, annotations_json)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                "#,
                params![
                    layer.manifest_digest,
                    i64::from(layer.position),
                    layer.digest,
                    layer.media_type,
                    i64::try_from(layer.size).context("Layer size does not fit in i64")?,
                    layer.annotations_json,
                ],
            )?;
        }
        Ok(())
    }

    pub fn get_manifest(&self, digest: &str) -> Result<Option<ManifestRecord>> {
        validate_digest(digest)?;
        self.lock()
            .query_row(
                r#"
                SELECT digest, media_type, size, subject_digest, annotations_json, created_at
                FROM manifests
                WHERE digest = ?1
                "#,
                params![digest],
                |row| {
                    Ok(ManifestRecord {
                        digest: row.get(0)?,
                        media_type: row.get(1)?,
                        size: read_u64(row, 2)?,
                        subject_digest: row.get(3)?,
                        annotations_json: row.get(4)?,
                        created_at: row.get(5)?,
                    })
                },
            )
            .optional()
            .context("Failed to query manifest record")
    }

    pub fn get_layers(&self, manifest_digest: &str) -> Result<Vec<LayerRecord>> {
        validate_digest(manifest_digest)?;
        let conn = self.lock();
        let mut stmt = conn.prepare(
            r#"
            SELECT manifest_digest, position, digest, media_type, size, annotations_json
            FROM manifest_layers
            WHERE manifest_digest = ?1
            ORDER BY position
            "#,
        )?;
        let rows = stmt.query_map(params![manifest_digest], |row| {
            Ok(LayerRecord {
                manifest_digest: row.get(0)?,
                position: read_u32(row, 1)?,
                digest: row.get(2)?,
                media_type: row.get(3)?,
                size: read_u64(row, 4)?,
                annotations_json: row.get(5)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn put_ref(&self, name: &str, reference: &str, manifest_digest: &str) -> Result<()> {
        let conn = self.lock();
        Self::put_ref_in(&conn, name, reference, manifest_digest)
    }

    fn put_ref_in(
        conn: &Connection,
        name: &str,
        reference: &str,
        manifest_digest: &str,
    ) -> Result<()> {
        validate_digest(manifest_digest)?;
        conn.execute(
            r#"
            INSERT INTO refs (name, reference, manifest_digest, updated_at)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(name, reference) DO UPDATE SET
                manifest_digest = excluded.manifest_digest,
                updated_at = excluded.updated_at
            "#,
            params![name, reference, manifest_digest, now_rfc3339()],
        )?;
        Ok(())
    }

    pub fn put_ref_with_policy(
        &self,
        name: &str,
        reference: &str,
        manifest_digest: &str,
        policy: RefConflictPolicy,
    ) -> Result<RefUpdate> {
        let conn = self.lock();
        Self::put_ref_with_policy_in(&conn, name, reference, manifest_digest, policy)
    }

    /// Connection-scoped variant of [`Self::put_ref_with_policy`].
    /// Lets callers compose ref publish with blob / manifest inserts
    /// inside one transaction (see [`Self::publish_artifact_atomic`]).
    fn put_ref_with_policy_in(
        conn: &Connection,
        name: &str,
        reference: &str,
        manifest_digest: &str,
        policy: RefConflictPolicy,
    ) -> Result<RefUpdate> {
        validate_digest(manifest_digest)?;
        if policy == RefConflictPolicy::Replace {
            return Self::replace_ref_in(conn, name, reference, manifest_digest);
        }

        let inserted = conn.execute(
            r#"
            INSERT INTO refs (name, reference, manifest_digest, updated_at)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(name, reference) DO NOTHING
            "#,
            params![name, reference, manifest_digest, now_rfc3339()],
        )?;
        if inserted == 1 {
            return Ok(RefUpdate::Inserted);
        }

        let existing_manifest_digest =
            Self::resolve_ref_in(conn, name, reference)?.with_context(|| {
                format!("Ref disappeared while resolving conflict: {name}:{reference}")
            })?;
        if existing_manifest_digest == manifest_digest {
            Ok(RefUpdate::Unchanged)
        } else {
            Ok(RefUpdate::Conflicted {
                existing_manifest_digest,
                incoming_manifest_digest: manifest_digest.to_string(),
            })
        }
    }

    fn replace_ref_in(
        conn: &Connection,
        name: &str,
        reference: &str,
        manifest_digest: &str,
    ) -> Result<RefUpdate> {
        let previous_manifest_digest = Self::resolve_ref_in(conn, name, reference)?;
        if previous_manifest_digest.as_deref() == Some(manifest_digest) {
            return Ok(RefUpdate::Unchanged);
        }

        Self::put_ref_in(conn, name, reference, manifest_digest)?;
        Ok(match previous_manifest_digest {
            Some(previous_manifest_digest) => RefUpdate::Replaced {
                previous_manifest_digest,
            },
            None => RefUpdate::Inserted,
        })
    }

    fn resolve_ref_in(conn: &Connection, name: &str, reference: &str) -> Result<Option<String>> {
        conn.query_row(
            "SELECT manifest_digest FROM refs WHERE name = ?1 AND reference = ?2",
            params![name, reference],
            |row| row.get(0),
        )
        .optional()
        .context("Failed to resolve local registry ref")
    }

    pub fn resolve_ref(&self, name: &str, reference: &str) -> Result<Option<String>> {
        let conn = self.lock();
        Self::resolve_ref_in(&conn, name, reference)
    }

    /// Atomic publish: in one SQLite transaction, insert all `blobs`
    /// records, the `manifest` + `layers`, and (if `image_name` is
    /// `Some`) the corresponding ref under `policy`. CAS bytes must
    /// already have been written to [`super::FileBlobStore`] before
    /// this call — only the IndexStore rows are batched here.
    ///
    /// On a `Replace` policy the function still returns
    /// `RefUpdate::Replaced { previous_manifest_digest }` so callers
    /// can record the prior state. A `Conflicted` outcome under
    /// `KeepExisting` aborts the transaction (so the manifest /
    /// layers / blob rows are not committed) and returns
    /// `Ok(PublishOutcome { ref_update: Some(Conflicted { .. }) })`;
    /// callers that prefer to bail can match on the variant.
    pub fn publish_artifact_atomic(
        &self,
        blobs: &[BlobRecord],
        manifest: &ManifestRecord,
        layers: &[LayerRecord],
        image_name: Option<&ImageName>,
        policy: RefConflictPolicy,
    ) -> Result<PublishOutcome> {
        // BEGIN IMMEDIATE acquires the RESERVED lock at the start so
        // `busy_timeout` waits cleanly when another writer is active,
        // instead of upgrading mid-transaction and risking SQLITE_BUSY
        // from two writers each holding a SHARED lock.
        let mut conn = self.lock();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        for blob in blobs {
            Self::put_blob_in(&tx, blob)?;
        }
        Self::put_manifest_in(&tx, manifest, layers)?;
        let ref_update = if let Some(image_name) = image_name {
            let name = image_name_repository(image_name);
            let reference = image_name.reference.as_str();
            let update =
                Self::put_ref_with_policy_in(&tx, &name, reference, &manifest.digest, policy)?;
            // KeepExisting + different incoming digest → conflict.
            // Roll back the manifest / blob inserts so we don't leave
            // unreferenced rows committed under a ref that resolved
            // to a different artifact.
            if let RefUpdate::Conflicted { .. } = &update {
                drop(tx);
                return Ok(PublishOutcome {
                    ref_update: Some(update),
                });
            }
            Some(update)
        } else {
            None
        };
        tx.commit()?;
        Ok(PublishOutcome { ref_update })
    }

    pub fn list_refs(&self, name_prefix: Option<&str>) -> Result<Vec<RefRecord>> {
        let conn = self.lock();
        let mut out = Vec::new();
        if let Some(prefix) = name_prefix {
            let prefix_len = i64::try_from(prefix.chars().count())
                .context("Ref prefix length does not fit in i64")?;
            let mut stmt = conn.prepare(
                r#"
                SELECT name, reference, manifest_digest, updated_at
                FROM refs
                WHERE substr(name, 1, ?1) = ?2
                ORDER BY name, reference
                "#,
            )?;
            let rows = stmt.query_map(params![prefix_len, prefix], ref_from_row)?;
            for row in rows {
                out.push(row?);
            }
        } else {
            let mut stmt = conn.prepare(
                r#"
                SELECT name, reference, manifest_digest, updated_at
                FROM refs
                ORDER BY name, reference
                "#,
            )?;
            let rows = stmt.query_map([], ref_from_row)?;
            for row in rows {
                out.push(row?);
            }
        }
        Ok(out)
    }

    fn init_schema(&self) -> Result<()> {
        self.lock().execute_batch(
            r#"
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS ommx_local_registry_schema (
                version INTEGER NOT NULL
            );

            INSERT INTO ommx_local_registry_schema (version)
            SELECT 1
            WHERE NOT EXISTS (SELECT 1 FROM ommx_local_registry_schema);

            CREATE TABLE IF NOT EXISTS blobs (
                digest TEXT PRIMARY KEY,
                size INTEGER NOT NULL CHECK(size >= 0),
                media_type TEXT,
                storage_uri TEXT NOT NULL,
                kind TEXT NOT NULL,
                created_at TEXT NOT NULL,
                last_verified_at TEXT
            );

            CREATE TABLE IF NOT EXISTS manifests (
                digest TEXT PRIMARY KEY,
                media_type TEXT NOT NULL,
                size INTEGER NOT NULL CHECK(size >= 0),
                subject_digest TEXT,
                annotations_json TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL,
                FOREIGN KEY(digest) REFERENCES blobs(digest)
            );

            CREATE TABLE IF NOT EXISTS manifest_layers (
                manifest_digest TEXT NOT NULL,
                position INTEGER NOT NULL CHECK(position >= 0),
                digest TEXT NOT NULL,
                media_type TEXT NOT NULL,
                size INTEGER NOT NULL CHECK(size >= 0),
                annotations_json TEXT NOT NULL DEFAULT '{}',
                PRIMARY KEY(manifest_digest, position),
                FOREIGN KEY(manifest_digest) REFERENCES manifests(digest) ON DELETE CASCADE,
                FOREIGN KEY(digest) REFERENCES blobs(digest)
            );

            CREATE TABLE IF NOT EXISTS refs (
                name TEXT NOT NULL,
                reference TEXT NOT NULL,
                manifest_digest TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                PRIMARY KEY(name, reference),
                FOREIGN KEY(manifest_digest) REFERENCES manifests(digest)
            );

            CREATE INDEX IF NOT EXISTS idx_refs_name ON refs(name);
            CREATE INDEX IF NOT EXISTS idx_manifest_layers_digest ON manifest_layers(digest);
            "#,
        )?;
        let version = self.schema_version()?;
        ensure!(
            version == SCHEMA_VERSION,
            "Unsupported local registry schema version: {version}"
        );
        Ok(())
    }
}

impl SqliteIndexStore {
    pub fn put_image_ref(&self, image_name: &ImageName, manifest_digest: &str) -> Result<()> {
        self.put_ref(
            &image_name_repository(image_name),
            image_name.reference.as_str(),
            manifest_digest,
        )
    }

    pub fn put_image_ref_with_policy(
        &self,
        image_name: &ImageName,
        manifest_digest: &str,
        policy: RefConflictPolicy,
    ) -> Result<RefUpdate> {
        self.put_ref_with_policy(
            &image_name_repository(image_name),
            image_name.reference.as_str(),
            manifest_digest,
            policy,
        )
    }

    pub fn resolve_image_name(&self, image_name: &ImageName) -> Result<Option<String>> {
        self.resolve_ref(
            &image_name_repository(image_name),
            image_name.reference.as_str(),
        )
    }
}

fn image_name_repository(image_name: &ImageName) -> String {
    if let Some(port) = image_name.port {
        format!("{}:{}/{}", image_name.hostname, port, image_name.name)
    } else {
        format!("{}/{}", image_name.hostname, image_name.name)
    }
}

fn ref_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RefRecord> {
    Ok(RefRecord {
        name: row.get(0)?,
        reference: row.get(1)?,
        manifest_digest: row.get(2)?,
        updated_at: row.get(3)?,
    })
}

fn read_u64(row: &rusqlite::Row<'_>, idx: usize) -> rusqlite::Result<u64> {
    let value: i64 = row.get(idx)?;
    u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(idx, value))
}

fn read_u32(row: &rusqlite::Row<'_>, idx: usize) -> rusqlite::Result<u32> {
    let value: i64 = row.get(idx)?;
    u32::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(idx, value))
}
