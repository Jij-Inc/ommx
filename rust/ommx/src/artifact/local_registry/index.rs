use super::{now_rfc3339, validate_digest, RefRecord, RefUpdate, SQLITE_INDEX_FILE_NAME};
use crate::artifact::ImageRef;
use anyhow::{bail, ensure, Context, Result};
use oci_spec::image::{Descriptor, DescriptorBuilder, Digest, MediaType};
use rusqlite::{params, types::Type, Connection, OptionalExtension, TransactionBehavior};
use std::{
    collections::HashMap,
    fs,
    path::Path,
    str::FromStr,
    sync::{Mutex, MutexGuard},
    time::Duration,
};

/// SQLite Local Registry schema version stored in `PRAGMA user_version`.
/// The SQLite Local Registry has not been released yet, so incompatible
/// development schemas fail fast instead of carrying in-place migrations.
const SCHEMA_VERSION: i64 = 1;
const SQLITE_BUSY_TIMEOUT: Duration = Duration::from_secs(30);

/// SQLite-backed index store for the v3 Local Registry.
///
/// This store is the concurrency-safe equivalent of an OCI `index.json`:
/// it stores refs and their target manifest descriptors. Content bytes
/// live in [`super::FileBlobStore`], and manifests / layers are read
/// from that CAS by descriptor digest instead of being cached here.
///
/// `rusqlite::Connection` is `Send` but `!Sync`, so it lives behind a
/// [`Mutex`] here. That makes [`SqliteIndexStore`] (and the enclosing
/// [`super::LocalRegistry`]) `Sync`, which lets the process-wide
/// default registry be shared as a `&'static LocalRegistry` across
/// PyO3 wrappers without per-artifact connection duplication.
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
    /// internal state isn't damaged by Rust-level poisoning; the lock
    /// flag just records that some other thread panicked while holding
    /// it) and log a warning.
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
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .context("Failed to read local registry schema version")
    }

    pub fn replace_ref(
        &self,
        name: &str,
        reference: &str,
        descriptor: &Descriptor,
    ) -> Result<RefUpdate> {
        let mut conn = self.lock();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let update = Self::replace_ref_in(&tx, name, reference, descriptor)?;
        tx.commit()?;
        Ok(update)
    }

    fn upsert_ref_in(
        conn: &Connection,
        name: &str,
        reference: &str,
        descriptor: &Descriptor,
    ) -> Result<()> {
        validate_digest(descriptor.digest().as_ref())?;
        let annotations_json = descriptor_annotations_json(descriptor)?;
        conn.execute(
            r#"
            INSERT INTO refs (
                name,
                reference,
                manifest_media_type,
                manifest_digest,
                manifest_size,
                manifest_annotations_json,
                updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(name, reference) DO UPDATE SET
                manifest_media_type = excluded.manifest_media_type,
                manifest_digest = excluded.manifest_digest,
                manifest_size = excluded.manifest_size,
                manifest_annotations_json = excluded.manifest_annotations_json,
                updated_at = excluded.updated_at
            "#,
            params![
                name,
                reference,
                descriptor.media_type().to_string(),
                descriptor.digest().to_string(),
                i64::try_from(descriptor.size()).context("Manifest size does not fit in i64")?,
                annotations_json,
                now_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn publish_ref(
        &self,
        name: &str,
        reference: &str,
        descriptor: &Descriptor,
    ) -> Result<RefUpdate> {
        let mut conn = self.lock();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let update = Self::publish_ref_in(&tx, name, reference, descriptor)?;
        tx.commit()?;
        Ok(update)
    }

    fn publish_ref_in(
        conn: &Connection,
        name: &str,
        reference: &str,
        descriptor: &Descriptor,
    ) -> Result<RefUpdate> {
        validate_digest(descriptor.digest().as_ref())?;
        let annotations_json = descriptor_annotations_json(descriptor)?;
        let inserted = conn.execute(
            r#"
            INSERT INTO refs (
                name,
                reference,
                manifest_media_type,
                manifest_digest,
                manifest_size,
                manifest_annotations_json,
                updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(name, reference) DO NOTHING
            "#,
            params![
                name,
                reference,
                descriptor.media_type().to_string(),
                descriptor.digest().to_string(),
                i64::try_from(descriptor.size()).context("Manifest size does not fit in i64")?,
                annotations_json,
                now_rfc3339(),
            ],
        )?;
        if inserted == 1 {
            return Ok(RefUpdate::Inserted);
        }

        let existing_descriptor =
            Self::resolve_ref_in(conn, name, reference)?.with_context(|| {
                format!("Ref disappeared while resolving conflict: {name}:{reference}")
            })?;
        let existing_manifest_digest = existing_descriptor.digest().clone();
        let incoming_manifest_digest = descriptor.digest().clone();
        if existing_manifest_digest == incoming_manifest_digest {
            Ok(RefUpdate::Unchanged)
        } else {
            Ok(RefUpdate::Conflicted {
                existing_manifest_digest,
                incoming_manifest_digest,
            })
        }
    }

    fn replace_ref_in(
        conn: &Connection,
        name: &str,
        reference: &str,
        descriptor: &Descriptor,
    ) -> Result<RefUpdate> {
        let previous_descriptor = Self::resolve_ref_in(conn, name, reference)?;
        if previous_descriptor.as_ref().map(|d| d.digest()) == Some(descriptor.digest()) {
            return Ok(RefUpdate::Unchanged);
        }

        Self::upsert_ref_in(conn, name, reference, descriptor)?;
        Ok(match previous_descriptor {
            Some(previous_descriptor) => RefUpdate::Replaced {
                previous_manifest_digest: previous_descriptor.digest().clone(),
            },
            None => RefUpdate::Inserted,
        })
    }

    fn resolve_ref_in(
        conn: &Connection,
        name: &str,
        reference: &str,
    ) -> Result<Option<Descriptor>> {
        conn.query_row(
            r#"
            SELECT manifest_media_type, manifest_digest, manifest_size, manifest_annotations_json
            FROM refs
            WHERE name = ?1 AND reference = ?2
            "#,
            params![name, reference],
            descriptor_from_row,
        )
        .optional()
        .context("Failed to resolve local registry ref")
    }

    pub fn resolve_ref(&self, name: &str, reference: &str) -> Result<Option<Descriptor>> {
        let conn = self.lock();
        Self::resolve_ref_in(&conn, name, reference)
    }

    /// Per-registry stable identifier. Generated as a random UUID v4
    /// (32 hex chars) on the first `init_schema` call and stored
    /// verbatim in the `ommx_local_registry_metadata` table.
    pub fn registry_id(&self) -> Result<String> {
        let conn = self.lock();
        let row: Option<String> = conn
            .query_row(
                r#"SELECT value FROM ommx_local_registry_metadata WHERE key = 'registry_id'"#,
                [],
                |r| r.get(0),
            )
            .optional()?;
        match row {
            Some(id) => Ok(id),
            None => {
                let new_id = random_registry_id();
                conn.execute(
                    r#"INSERT OR IGNORE INTO ommx_local_registry_metadata (key, value)
                       VALUES ('registry_id', ?1)"#,
                    params![&new_id],
                )?;
                conn.query_row(
                    r#"SELECT value FROM ommx_local_registry_metadata WHERE key = 'registry_id'"#,
                    [],
                    |r| r.get::<_, String>(0),
                )
                .context("Failed to read registry_id after insert")
            }
        }
    }

    /// Delete a single ref row by `(name, reference)`. Returns `true`
    /// when a row was actually removed. Content blobs are not touched;
    /// unreferenced CAS bytes are reclaimed by a future GC sweep, not
    /// by this primitive.
    pub fn delete_ref(&self, name: &str, reference: &str) -> Result<bool> {
        let mut conn = self.lock();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let affected = tx.execute(
            r#"DELETE FROM refs WHERE name = ?1 AND reference = ?2"#,
            params![name, reference],
        )?;
        tx.commit()?;
        Ok(affected > 0)
    }

    pub fn list_refs(&self, name_prefix: Option<&str>) -> Result<Vec<RefRecord>> {
        let conn = self.lock();
        let mut out = Vec::new();
        if let Some(prefix) = name_prefix {
            let prefix_len = i64::try_from(prefix.chars().count())
                .context("Ref prefix length does not fit in i64")?;
            let mut stmt = conn.prepare(
                r#"
                SELECT
                    name,
                    reference,
                    manifest_media_type,
                    manifest_digest,
                    manifest_size,
                    manifest_annotations_json,
                    updated_at
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
                SELECT
                    name,
                    reference,
                    manifest_media_type,
                    manifest_digest,
                    manifest_size,
                    manifest_annotations_json,
                    updated_at
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
        let conn = self.lock();
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;

        let version = conn
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .context("Failed to read local registry schema version")?;
        if version == 0 {
            if has_user_tables(&conn)? {
                bail!(
                    "Unsupported local registry schema version: 0. \
                     Remove the development local registry and recreate it."
                );
            }
            // Mark the fresh database before creating tables. Multiple
            // processes may open a new registry concurrently; setting
            // `user_version` first prevents another opener from
            // observing newly-created tables while the version still
            // reads as 0 and misclassifying the registry as an old
            // development schema.
            conn.pragma_update(None, "user_version", SCHEMA_VERSION)
                .context("Failed to initialize local registry schema version")?;
        } else {
            ensure!(
                version == SCHEMA_VERSION,
                "Unsupported local registry schema version: {version}"
            );
        }

        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS ommx_local_registry_metadata (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS refs (
                name TEXT NOT NULL,
                reference TEXT NOT NULL,
                manifest_media_type TEXT NOT NULL,
                manifest_digest TEXT NOT NULL,
                manifest_size INTEGER NOT NULL CHECK(manifest_size >= 0),
                manifest_annotations_json TEXT NOT NULL DEFAULT '{}',
                updated_at TEXT NOT NULL,
                PRIMARY KEY(name, reference)
            );

            CREATE INDEX IF NOT EXISTS idx_refs_name ON refs(name);
            "#,
        )?;
        Ok(())
    }
}

impl SqliteIndexStore {
    pub fn publish_image_ref(
        &self,
        image_name: &ImageRef,
        descriptor: &Descriptor,
    ) -> Result<RefUpdate> {
        self.publish_ref(
            &image_name.repository_key(),
            image_name.reference(),
            descriptor,
        )
    }

    pub fn replace_image_ref(
        &self,
        image_name: &ImageRef,
        descriptor: &Descriptor,
    ) -> Result<RefUpdate> {
        self.replace_ref(
            &image_name.repository_key(),
            image_name.reference(),
            descriptor,
        )
    }

    pub fn resolve_image_descriptor(&self, image_name: &ImageRef) -> Result<Option<Descriptor>> {
        self.resolve_ref(&image_name.repository_key(), image_name.reference())
    }

    pub fn resolve_image_name(&self, image_name: &ImageRef) -> Result<Option<Digest>> {
        Ok(self
            .resolve_image_descriptor(image_name)?
            .map(|descriptor| descriptor.digest().clone()))
    }
}

fn ref_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RefRecord> {
    Ok(RefRecord {
        name: row.get(0)?,
        reference: row.get(1)?,
        descriptor: descriptor_from_ref_row(row, 2)?,
        updated_at: row.get(6)?,
    })
}

fn descriptor_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Descriptor> {
    descriptor_from_ref_row(row, 0)
}

fn has_user_tables(conn: &Connection) -> Result<bool> {
    let count: i64 = conn
        .query_row(
            r#"
            SELECT COUNT(*)
            FROM sqlite_master
            WHERE type = 'table'
              AND name NOT LIKE 'sqlite_%'
            "#,
            [],
            |row| row.get(0),
        )
        .context("Failed to inspect local registry schema tables")?;
    Ok(count > 0)
}

fn descriptor_from_ref_row(row: &rusqlite::Row<'_>, offset: usize) -> rusqlite::Result<Descriptor> {
    let media_type: String = row.get(offset)?;
    let digest: String = row.get(offset + 1)?;
    let size = read_u64(row, offset + 2)?;
    let annotations_json: String = row.get(offset + 3)?;
    let annotations = parse_annotations_json(&annotations_json, offset + 3)?;
    let digest = Digest::from_str(&digest).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(offset + 1, Type::Text, Box::new(err))
    })?;
    let mut builder = DescriptorBuilder::default()
        .media_type(media_type_from_string(media_type))
        .digest(digest)
        .size(size);
    if !annotations.is_empty() {
        builder = builder.annotations(annotations);
    }
    builder
        .build()
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(offset, Type::Text, err.into()))
}

fn descriptor_annotations_json(descriptor: &Descriptor) -> Result<String> {
    match descriptor.annotations() {
        Some(annotations) => String::from_utf8(crate::artifact::stable_json_bytes(annotations)?)
            .context("Stable descriptor annotation JSON is not UTF-8"),
        None => Ok("{}".to_string()),
    }
}

fn parse_annotations_json(
    json: &str,
    column_index: usize,
) -> rusqlite::Result<HashMap<String, String>> {
    serde_json::from_str(json).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(column_index, Type::Text, Box::new(err))
    })
}

fn media_type_from_string(media_type: String) -> MediaType {
    if media_type == MediaType::ImageManifest.to_string() {
        MediaType::ImageManifest
    } else if media_type == MediaType::ImageIndex.to_string() {
        MediaType::ImageIndex
    } else if media_type == MediaType::EmptyJSON.to_string() {
        MediaType::EmptyJSON
    } else if media_type == MediaType::ArtifactManifest.to_string() {
        MediaType::ArtifactManifest
    } else {
        MediaType::Other(media_type)
    }
}

fn read_u64(row: &rusqlite::Row<'_>, idx: usize) -> rusqlite::Result<u64> {
    let value: i64 = row.get(idx)?;
    u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(idx, value))
}

fn random_registry_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}
