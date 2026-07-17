use super::{
    now_rfc3339, ArtifactManifestRecord, ExperimentManifestRecord, ExperimentRefRecord, RefRecord,
    RefUpdate, SQLITE_INDEX_FILE_NAME,
};
use crate::artifact::digest::validate_digest;
use crate::artifact::media_types;
use crate::artifact::{sha256_digest, ImageRef};
use crate::experiment::config::ExperimentConfig;
use anyhow::{bail, ensure, Context, Result};
#[cfg(test)]
use oci_spec::image::DescriptorBuilder;
use oci_spec::image::{Descriptor, Digest, ImageManifest, MediaType};
use rusqlite::{params, types::Type, Connection, OptionalExtension, TransactionBehavior};
use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    str::FromStr,
    sync::{Mutex, MutexGuard},
    time::Duration,
};

/// SQLite Local Registry schema version stored in `PRAGMA user_version`.
const SCHEMA_VERSION: i64 = 2;
const SQLITE_BUSY_TIMEOUT: Duration = Duration::from_secs(30);

/// Immutable Local Registry listing record for an OMMX Artifact.
///
/// Values are reconstructed only by the SQLite index from a ref and the
/// digest-addressed copy of its original OCI Manifest JSON. The Manifest bytes
/// are verified against `manifest_digest` before this record is returned. Use
/// [`Self::manifest_digest`] as the immutable artifact identity;
/// [`Self::image_name`] is the mutable Local Registry alias that pointed to it
/// when the snapshot was read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactRefRecord {
    image_name: ImageRef,
    manifest_digest: Digest,
    updated_at: String,
    manifest_size: u64,
    manifest: ImageManifest,
}

impl ArtifactRefRecord {
    /// Full Local Registry image reference.
    pub fn image_name(&self) -> &ImageRef {
        &self.image_name
    }

    /// Immutable OCI Manifest digest for the Artifact.
    pub fn manifest_digest(&self) -> &Digest {
        &self.manifest_digest
    }

    /// RFC 3339 timestamp when the Local Registry ref was last updated.
    pub fn updated_at(&self) -> &str {
        &self.updated_at
    }

    /// Byte length of the original OCI Manifest JSON.
    pub fn manifest_size(&self) -> u64 {
        self.manifest_size
    }

    /// Parsed OCI Image Manifest stored by [`Self::manifest_digest`].
    pub fn manifest(&self) -> &ImageManifest {
        &self.manifest
    }

    /// Sum of the sizes declared directly by this Artifact's OCI Manifest.
    ///
    /// This includes the original Manifest JSON bytes, its config descriptor,
    /// and each unique layer digest. The OCI `subject` is intentionally excluded.
    /// Blobs shared with other Artifact refs are counted for each ref, so values
    /// from multiple records are not additive physical Local Registry usage.
    pub fn referenced_blob_size(&self) -> Result<u64> {
        let mut blob_sizes = BTreeMap::new();
        for descriptor in std::iter::once(self.manifest.config()).chain(self.manifest.layers()) {
            let digest = descriptor.digest().to_string();
            if let Some(previous_size) = blob_sizes.insert(digest.clone(), descriptor.size()) {
                ensure!(
                    previous_size == descriptor.size(),
                    "Manifest contains conflicting sizes for {digest}: {previous_size} and {}",
                    descriptor.size()
                );
            }
        }

        blob_sizes
            .into_values()
            .try_fold(self.manifest_size, |total, size| {
                total
                    .checked_add(size)
                    .context("Referenced blob size overflowed u64")
            })
    }
}

/// SQLite-backed index store for the v3 Local Registry.
///
/// This store is the concurrency-safe equivalent of an OCI `index.json`:
/// it stores refs and their target manifest digest. Content bytes live in
/// the Local Registry content-addressed storage. The index also keeps
/// digest-addressed copies of the original Manifest and Experiment Config JSON
/// used for blob-free catalog listings; those rows are caches of CAS-addressed
/// bytes, not a second editable source of truth.
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

pub struct CachedRefIdentity {
    pub name: String,
    pub reference: String,
    pub manifest_digest: String,
    pub parsed: std::result::Result<(ImageRef, Digest), String>,
}

impl CachedRefIdentity {
    pub fn image_name(&self) -> String {
        let separator = if self.reference.contains(':') {
            '@'
        } else {
            ':'
        };
        format!("{}{separator}{}", self.name, self.reference)
    }
}

pub struct CachedRefRead<T> {
    pub identity: CachedRefIdentity,
    pub record: std::result::Result<T, String>,
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

    #[cfg(test)]
    pub fn schema_version(&self) -> Result<i64> {
        self.lock()
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .context("Failed to read local registry schema version")
    }

    #[cfg(test)]
    pub fn replace_ref(
        &self,
        name: &str,
        reference: &str,
        descriptor: &Descriptor,
    ) -> Result<RefUpdate> {
        let mut conn = self.lock();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let update = Self::replace_ref_in(&tx, name, reference, descriptor)?;
        Self::prune_unreferenced_cache_in(&tx)?;
        tx.commit()?;
        Ok(update)
    }

    pub fn publish_experiment_ref(
        &self,
        image_name: &ImageRef,
        descriptor: &Descriptor,
        experiment: &ExperimentManifestRecord,
    ) -> Result<RefUpdate> {
        Self::ensure_artifact_descriptor_matches_ref_descriptor(descriptor, experiment.artifact())?;
        let mut conn = self.lock();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let update = Self::publish_ref_in(
            &tx,
            &image_name.repository_key(),
            image_name.reference(),
            descriptor,
        )?;
        if !matches!(update, RefUpdate::Conflicted { .. }) {
            Self::upsert_experiment_manifest_in(&tx, experiment)?;
        }
        tx.commit()?;
        Ok(update)
    }

    pub fn replace_experiment_ref(
        &self,
        image_name: &ImageRef,
        descriptor: &Descriptor,
        experiment: &ExperimentManifestRecord,
    ) -> Result<RefUpdate> {
        Self::ensure_artifact_descriptor_matches_ref_descriptor(descriptor, experiment.artifact())?;
        let mut conn = self.lock();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let update = Self::replace_ref_in(
            &tx,
            &image_name.repository_key(),
            image_name.reference(),
            descriptor,
        )?;
        Self::upsert_experiment_manifest_in(&tx, experiment)?;
        Self::prune_unreferenced_cache_in(&tx)?;
        tx.commit()?;
        Ok(update)
    }

    pub fn publish_artifact_ref(
        &self,
        image_name: &ImageRef,
        descriptor: &Descriptor,
        artifact: &ArtifactManifestRecord,
    ) -> Result<RefUpdate> {
        Self::ensure_artifact_descriptor_matches_ref_descriptor(descriptor, artifact)?;
        let mut conn = self.lock();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let update = Self::publish_ref_in(
            &tx,
            &image_name.repository_key(),
            image_name.reference(),
            descriptor,
        )?;
        if !matches!(update, RefUpdate::Conflicted { .. }) {
            Self::upsert_artifact_manifest_in(&tx, artifact)?;
        }
        tx.commit()?;
        Ok(update)
    }

    pub fn replace_artifact_ref(
        &self,
        image_name: &ImageRef,
        descriptor: &Descriptor,
        artifact: &ArtifactManifestRecord,
    ) -> Result<RefUpdate> {
        Self::ensure_artifact_descriptor_matches_ref_descriptor(descriptor, artifact)?;
        let mut conn = self.lock();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let update = Self::replace_ref_in(
            &tx,
            &image_name.repository_key(),
            image_name.reference(),
            descriptor,
        )?;
        Self::upsert_artifact_manifest_in(&tx, artifact)?;
        Self::prune_unreferenced_cache_in(&tx)?;
        tx.commit()?;
        Ok(update)
    }

    pub fn upsert_experiment_manifest(&self, experiment: &ExperimentManifestRecord) -> Result<()> {
        let mut conn = self.lock();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        Self::upsert_experiment_manifest_in(&tx, experiment)?;
        Self::prune_unreferenced_cache_in(&tx)?;
        tx.commit()?;
        Ok(())
    }

    pub fn upsert_artifact_manifest(&self, artifact: &ArtifactManifestRecord) -> Result<()> {
        let mut conn = self.lock();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        Self::upsert_artifact_manifest_in(&tx, artifact)?;
        Self::prune_unreferenced_cache_in(&tx)?;
        tx.commit()?;
        Ok(())
    }

    fn upsert_experiment_manifest_in(
        conn: &Connection,
        experiment: &ExperimentManifestRecord,
    ) -> Result<()> {
        Self::upsert_artifact_manifest_in(conn, experiment.artifact())?;
        Self::upsert_experiment_config_in(conn, experiment)
    }

    fn upsert_artifact_manifest_in(
        conn: &Connection,
        artifact: &ArtifactManifestRecord,
    ) -> Result<()> {
        validate_digest(artifact.manifest_digest().as_ref())?;
        validate_digest(artifact.config_digest().as_ref())?;
        ensure!(
            artifact.manifest_digest().as_ref() == sha256_digest(artifact.manifest_json()),
            "Artifact manifest cache digest mismatch for {}",
            artifact.manifest_digest()
        );
        conn.execute(
            r#"
            INSERT INTO artifact_manifests (
                manifest_digest,
                manifest_json,
                artifact_type,
                config_digest
            )
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(manifest_digest) DO UPDATE SET
                manifest_json = excluded.manifest_json,
                artifact_type = excluded.artifact_type,
                config_digest = excluded.config_digest
            "#,
            params![
                artifact.manifest_digest().to_string(),
                artifact.manifest_json(),
                artifact.artifact_type().to_string(),
                artifact.config_digest().to_string(),
            ],
        )?;
        Ok(())
    }

    fn upsert_experiment_config_in(
        conn: &Connection,
        experiment: &ExperimentManifestRecord,
    ) -> Result<()> {
        let config_digest = experiment.artifact().config_digest();
        validate_digest(config_digest.as_ref())?;
        ensure!(
            config_digest.as_ref() == sha256_digest(experiment.config_json()),
            "Experiment config cache digest mismatch for {}",
            config_digest
        );
        conn.execute(
            r#"
            INSERT INTO experiment_configs (
                config_digest,
                config_json
            )
            VALUES (?1, ?2)
            ON CONFLICT(config_digest) DO UPDATE SET
                config_json = excluded.config_json
            "#,
            params![config_digest.to_string(), experiment.config_json(),],
        )?;
        Ok(())
    }

    fn prune_unreferenced_cache_in(conn: &Connection) -> Result<()> {
        conn.execute(
            r#"
            DELETE FROM artifact_manifests
            WHERE NOT EXISTS (
                SELECT 1
                FROM refs
                WHERE refs.manifest_digest = artifact_manifests.manifest_digest
            )
            "#,
            [],
        )?;
        conn.execute(
            r#"
            DELETE FROM experiment_configs
            WHERE NOT EXISTS (
                SELECT 1
                FROM artifact_manifests
                WHERE artifact_manifests.config_digest = experiment_configs.config_digest
            )
            "#,
            [],
        )?;
        Ok(())
    }

    fn ensure_artifact_descriptor_matches_ref_descriptor(
        descriptor: &Descriptor,
        artifact: &ArtifactManifestRecord,
    ) -> Result<()> {
        ensure!(
            descriptor.digest() == artifact.manifest_digest(),
            "Manifest cache digest {} does not match ref descriptor digest {}",
            artifact.manifest_digest(),
            descriptor.digest()
        );
        ensure!(
            descriptor.media_type() == &MediaType::ImageManifest,
            "Ref descriptor media type {} is not an OCI Image Manifest",
            descriptor.media_type()
        );
        ensure!(
            descriptor.size() == artifact.manifest_json().len() as u64,
            "Manifest cache size {} does not match ref descriptor size {}",
            artifact.manifest_json().len(),
            descriptor.size()
        );
        Ok(())
    }

    fn upsert_ref_in(
        conn: &Connection,
        name: &str,
        reference: &str,
        descriptor: &Descriptor,
    ) -> Result<()> {
        validate_digest(descriptor.digest().as_ref())?;
        conn.execute(
            r#"
            INSERT INTO refs (
                name,
                reference,
                manifest_digest,
                updated_at
            )
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(name, reference) DO UPDATE SET
                manifest_digest = excluded.manifest_digest,
                updated_at = excluded.updated_at
            "#,
            params![
                name,
                reference,
                descriptor.digest().to_string(),
                now_rfc3339(),
            ],
        )?;
        Ok(())
    }

    #[cfg(test)]
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
        let inserted = conn.execute(
            r#"
            INSERT INTO refs (
                name,
                reference,
                manifest_digest,
                updated_at
            )
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(name, reference) DO NOTHING
            "#,
            params![
                name,
                reference,
                descriptor.digest().to_string(),
                now_rfc3339(),
            ],
        )?;
        if inserted == 1 {
            return Ok(RefUpdate::Inserted);
        }

        let existing_manifest_digest = Self::resolve_ref_digest_in(conn, name, reference)?
            .with_context(|| {
                format!("Ref disappeared while resolving conflict: {name}:{reference}")
            })?;
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
        let previous_manifest_digest = Self::resolve_ref_digest_in(conn, name, reference)?;
        if previous_manifest_digest.as_ref() == Some(descriptor.digest()) {
            return Ok(RefUpdate::Unchanged);
        }

        Self::upsert_ref_in(conn, name, reference, descriptor)?;
        Ok(match previous_manifest_digest {
            Some(previous_manifest_digest) => RefUpdate::Replaced {
                previous_manifest_digest,
            },
            None => RefUpdate::Inserted,
        })
    }

    fn resolve_ref_digest_in(
        conn: &Connection,
        name: &str,
        reference: &str,
    ) -> Result<Option<Digest>> {
        conn.query_row(
            r#"
            SELECT manifest_digest
            FROM refs
            WHERE name = ?1 AND reference = ?2
            "#,
            params![name, reference],
            digest_from_row,
        )
        .optional()
        .context("Failed to resolve local registry ref digest")
    }

    #[cfg(test)]
    fn resolve_ref_in(
        conn: &Connection,
        name: &str,
        reference: &str,
    ) -> Result<Option<Descriptor>> {
        conn.query_row(
            r#"
            SELECT refs.manifest_digest, artifact_manifests.manifest_json
            FROM refs
            JOIN artifact_manifests
              ON refs.manifest_digest = artifact_manifests.manifest_digest
            WHERE name = ?1 AND reference = ?2
            "#,
            params![name, reference],
            descriptor_from_cached_manifest_row,
        )
        .optional()
        .context("Failed to resolve local registry ref")
    }

    #[cfg(test)]
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

    /// Delete a single ref row by `(name, reference)` and return the removed
    /// record needed to restore it. Content blobs are not touched;
    /// unreferenced CAS bytes are reclaimed by a future GC sweep, not
    /// by this primitive.
    pub fn delete_ref(&self, name: &str, reference: &str) -> Result<Option<RefRecord>> {
        let mut conn = self.lock();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let record = tx
            .query_row(
                r#"SELECT name, reference, manifest_digest, updated_at
                   FROM refs WHERE name = ?1 AND reference = ?2"#,
                params![name, reference],
                |row| {
                    Ok(RefRecord {
                        name: row.get(0)?,
                        reference: row.get(1)?,
                        manifest_digest: digest_from_column(row, 2)?,
                        updated_at: row.get(3)?,
                    })
                },
            )
            .optional()?;
        if record.is_some() {
            tx.execute(
                r#"DELETE FROM refs WHERE name = ?1 AND reference = ?2"#,
                params![name, reference],
            )?;
        }
        Self::prune_unreferenced_cache_in(&tx)?;
        tx.commit()?;
        Ok(record)
    }

    /// Delete candidate refs only while their digest and update timestamp
    /// still match the listing snapshot used to select them.
    ///
    /// This compare-and-delete boundary prevents a concurrent ref replacement
    /// from being removed by a prune pass that selected the older row.
    pub fn delete_refs_if_unchanged(&self, candidates: &[RefRecord]) -> Result<Vec<RefRecord>> {
        let mut conn = self.lock();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut removed = Vec::new();
        for candidate in candidates {
            let affected = tx.execute(
                r#"DELETE FROM refs
                   WHERE name = ?1
                     AND reference = ?2
                     AND manifest_digest = ?3
                     AND updated_at = ?4"#,
                params![
                    &candidate.name,
                    &candidate.reference,
                    candidate.manifest_digest.as_ref(),
                    &candidate.updated_at,
                ],
            )?;
            if affected > 0 {
                removed.push(candidate.clone());
            }
        }
        Self::prune_unreferenced_cache_in(&tx)?;
        tx.commit()?;
        Ok(removed)
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
                    manifest_digest,
                    updated_at
                FROM refs
                WHERE substr(
                    name ||
                    CASE WHEN instr(reference, ':') > 0 THEN '@' ELSE ':' END ||
                    reference,
                    1,
                    ?1
                ) = ?2
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
                    manifest_digest,
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

    pub fn list_missing_artifact_manifest_refs(
        &self,
        name_prefix: Option<&str>,
    ) -> Result<Vec<CachedRefIdentity>> {
        let conn = self.lock();
        let mut out = Vec::new();
        if let Some(prefix) = name_prefix {
            let prefix_len = i64::try_from(prefix.chars().count())
                .context("Ref prefix length does not fit in i64")?;
            let mut stmt = conn.prepare(
                r#"
                SELECT refs.name, refs.reference, refs.manifest_digest
                FROM refs
                LEFT JOIN artifact_manifests
                  ON refs.manifest_digest = artifact_manifests.manifest_digest
                WHERE artifact_manifests.manifest_digest IS NULL
                  AND substr(
                      refs.name ||
                      CASE WHEN instr(refs.reference, ':') > 0 THEN '@' ELSE ':' END ||
                      refs.reference,
                      1,
                      ?1
                  ) = ?2
                ORDER BY refs.name, refs.reference
                "#,
            )?;
            let rows = stmt.query_map(params![prefix_len, prefix], |row| {
                cached_ref_identity_from_row(row, 2)
            })?;
            for row in rows {
                out.push(row?);
            }
        } else {
            let mut stmt = conn.prepare(
                r#"
                SELECT refs.name, refs.reference, refs.manifest_digest
                FROM refs
                LEFT JOIN artifact_manifests
                  ON refs.manifest_digest = artifact_manifests.manifest_digest
                WHERE artifact_manifests.manifest_digest IS NULL
                ORDER BY refs.name, refs.reference
                "#,
            )?;
            let rows = stmt.query_map([], |row| cached_ref_identity_from_row(row, 2))?;
            for row in rows {
                out.push(row?);
            }
        }
        Ok(out)
    }

    pub fn list_artifact_ref_reads(
        &self,
        name_prefix: Option<&str>,
    ) -> Result<Vec<CachedRefRead<ArtifactRefRecord>>> {
        let conn = self.lock();
        let mut out = Vec::new();
        if let Some(prefix) = name_prefix {
            let prefix_len = i64::try_from(prefix.chars().count())
                .context("Ref prefix length does not fit in i64")?;
            let mut stmt = conn.prepare(
                r#"
                SELECT
                    refs.name,
                    refs.reference,
                    refs.updated_at,
                    artifact_manifests.manifest_digest,
                    artifact_manifests.config_digest,
                    artifact_manifests.artifact_type,
                    artifact_manifests.manifest_json
                FROM refs
                JOIN artifact_manifests
                  ON refs.manifest_digest = artifact_manifests.manifest_digest
                WHERE substr(
                    refs.name ||
                    CASE WHEN instr(refs.reference, ':') > 0 THEN '@' ELSE ':' END ||
                    refs.reference,
                    1,
                    ?1
                ) = ?2
                ORDER BY refs.name, refs.reference
                "#,
            )?;
            let rows = stmt.query_map(params![prefix_len, prefix], artifact_ref_read_from_row)?;
            for row in rows {
                out.push(row?);
            }
        } else {
            let mut stmt = conn.prepare(
                r#"
                SELECT
                    refs.name,
                    refs.reference,
                    refs.updated_at,
                    artifact_manifests.manifest_digest,
                    artifact_manifests.config_digest,
                    artifact_manifests.artifact_type,
                    artifact_manifests.manifest_json
                FROM refs
                JOIN artifact_manifests
                  ON refs.manifest_digest = artifact_manifests.manifest_digest
                ORDER BY refs.name, refs.reference
                "#,
            )?;
            let rows = stmt.query_map([], artifact_ref_read_from_row)?;
            for row in rows {
                out.push(row?);
            }
        }
        Ok(out)
    }

    pub fn list_missing_experiment_config_refs(
        &self,
        name_prefix: Option<&str>,
    ) -> Result<Vec<CachedRefIdentity>> {
        let conn = self.lock();
        let mut out = Vec::new();
        if let Some(prefix) = name_prefix {
            let prefix_len = i64::try_from(prefix.chars().count())
                .context("Ref prefix length does not fit in i64")?;
            let mut stmt = conn.prepare(
                r#"
                SELECT refs.name, refs.reference, artifact_manifests.manifest_digest
                FROM refs
                JOIN artifact_manifests
                  ON refs.manifest_digest = artifact_manifests.manifest_digest
                LEFT JOIN experiment_configs
                  ON artifact_manifests.config_digest = experiment_configs.config_digest
                WHERE artifact_manifests.artifact_type = ?3
                  AND experiment_configs.config_digest IS NULL
                  AND substr(
                    refs.name ||
                    CASE WHEN instr(refs.reference, ':') > 0 THEN '@' ELSE ':' END ||
                    refs.reference,
                    1,
                    ?1
                ) = ?2
                ORDER BY refs.name, refs.reference
                "#,
            )?;
            let rows = stmt.query_map(
                params![prefix_len, prefix, media_types::V1_EXPERIMENT_MEDIA_TYPE],
                |row| cached_ref_identity_from_row(row, 2),
            )?;
            for row in rows {
                out.push(row?);
            }
        } else {
            let mut stmt = conn.prepare(
                r#"
                SELECT refs.name, refs.reference, artifact_manifests.manifest_digest
                FROM refs
                JOIN artifact_manifests
                  ON refs.manifest_digest = artifact_manifests.manifest_digest
                LEFT JOIN experiment_configs
                  ON artifact_manifests.config_digest = experiment_configs.config_digest
                WHERE artifact_manifests.artifact_type = ?1
                  AND experiment_configs.config_digest IS NULL
                ORDER BY refs.name, refs.reference
                "#,
            )?;
            let rows = stmt.query_map(params![media_types::V1_EXPERIMENT_MEDIA_TYPE], |row| {
                cached_ref_identity_from_row(row, 2)
            })?;
            for row in rows {
                out.push(row?);
            }
        }
        Ok(out)
    }

    pub fn list_experiment_ref_reads(
        &self,
        name_prefix: Option<&str>,
    ) -> Result<Vec<CachedRefRead<ExperimentRefRecord>>> {
        let conn = self.lock();
        let mut out = Vec::new();
        if let Some(prefix) = name_prefix {
            let prefix_len = i64::try_from(prefix.chars().count())
                .context("Ref prefix length does not fit in i64")?;
            let mut stmt = conn.prepare(
                r#"
                SELECT
                    refs.name,
                    refs.reference,
                    refs.updated_at,
                    artifact_manifests.manifest_digest,
                    artifact_manifests.config_digest,
                    artifact_manifests.artifact_type,
                    artifact_manifests.manifest_json,
                    experiment_configs.config_json
                FROM refs
                JOIN artifact_manifests
                  ON refs.manifest_digest = artifact_manifests.manifest_digest
                JOIN experiment_configs
                  ON artifact_manifests.config_digest = experiment_configs.config_digest
                WHERE artifact_manifests.artifact_type = ?3
                  AND substr(
                    refs.name ||
                    CASE WHEN instr(refs.reference, ':') > 0 THEN '@' ELSE ':' END ||
                    refs.reference,
                    1,
                    ?1
                ) = ?2
                ORDER BY refs.name, refs.reference
                "#,
            )?;
            let rows = stmt.query_map(
                params![prefix_len, prefix, media_types::V1_EXPERIMENT_MEDIA_TYPE],
                experiment_ref_read_from_row,
            )?;
            for row in rows {
                out.push(row?);
            }
        } else {
            let mut stmt = conn.prepare(
                r#"
                SELECT
                    refs.name,
                    refs.reference,
                    refs.updated_at,
                    artifact_manifests.manifest_digest,
                    artifact_manifests.config_digest,
                    artifact_manifests.artifact_type,
                    artifact_manifests.manifest_json,
                    experiment_configs.config_json
                FROM refs
                JOIN artifact_manifests
                  ON refs.manifest_digest = artifact_manifests.manifest_digest
                JOIN experiment_configs
                  ON artifact_manifests.config_digest = experiment_configs.config_digest
                WHERE artifact_manifests.artifact_type = ?1
                ORDER BY refs.name, refs.reference
                "#,
            )?;
            let rows = stmt.query_map(
                params![media_types::V1_EXPERIMENT_MEDIA_TYPE],
                experiment_ref_read_from_row,
            )?;
            for row in rows {
                out.push(row?);
            }
        }
        Ok(out)
    }

    fn init_schema(&self) -> Result<()> {
        let mut conn = self.lock();
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let version = tx
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .context("Failed to read local registry schema version")?;
        match version {
            0 => {
                if has_user_tables(&tx)? {
                    bail!(
                        "Unsupported local registry schema version: 0. \
                         Remove the development local registry and recreate it."
                    );
                }
                create_schema(&tx)?;
            }
            SCHEMA_VERSION if has_current_schema(&tx)? => create_schema(&tx)?,
            1 | SCHEMA_VERSION => migrate_legacy_schema_to_v2(&tx)
                .context("Failed to migrate local registry schema to version 2")?,
            _ => {
                bail!(
                    "Unsupported local registry schema version: {version}; \
                     expected version {SCHEMA_VERSION}"
                );
            }
        }
        tx.pragma_update(None, "user_version", SCHEMA_VERSION)
            .context("Failed to update local registry schema version")?;
        tx.commit()?;
        Ok(())
    }
}

impl SqliteIndexStore {
    #[cfg(test)]
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

    #[cfg(test)]
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

    #[cfg(test)]
    pub fn resolve_image_descriptor(&self, image_name: &ImageRef) -> Result<Option<Descriptor>> {
        self.resolve_ref(&image_name.repository_key(), image_name.reference())
    }

    pub fn resolve_image_name(&self, image_name: &ImageRef) -> Result<Option<Digest>> {
        let conn = self.lock();
        Self::resolve_ref_digest_in(&conn, &image_name.repository_key(), image_name.reference())
    }
}

fn ref_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RefRecord> {
    let digest: String = row.get(2)?;
    let manifest_digest = Digest::from_str(&digest)
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(2, Type::Text, Box::new(err)))?;
    Ok(RefRecord {
        name: row.get(0)?,
        reference: row.get(1)?,
        manifest_digest,
        updated_at: row.get(3)?,
    })
}

fn artifact_ref_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ArtifactRefRecord> {
    let name: String = row.get(0)?;
    let reference: String = row.get(1)?;
    let image_name = ImageRef::from_repository_and_reference(&name, &reference)
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(0, Type::Text, err.into()))?;
    let digest: String = row.get(3)?;
    let manifest_digest = Digest::from_str(&digest)
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(3, Type::Text, Box::new(err)))?;
    let digest: String = row.get(4)?;
    let config_digest = Digest::from_str(&digest)
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(4, Type::Text, Box::new(err)))?;
    let artifact_type: String = row.get(5)?;
    let artifact_type = MediaType::from(artifact_type.as_str());
    if !media_types::is_ommx_artifact_type(&artifact_type) {
        return Err(cache_conversion_failure(
            5,
            Type::Text,
            format!("Unexpected cached OMMX artifact type: {artifact_type}"),
        ));
    }
    let manifest_json: Vec<u8> = row.get(6)?;
    if manifest_digest.as_ref() != sha256_digest(&manifest_json) {
        return Err(cache_conversion_failure(
            6,
            Type::Blob,
            format!("Cached Manifest JSON does not match {manifest_digest}"),
        ));
    }
    let manifest: ImageManifest = serde_json::from_slice(&manifest_json)
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(6, Type::Blob, Box::new(err)))?;
    if manifest.artifact_type().as_ref() != Some(&artifact_type) {
        return Err(cache_conversion_failure(
            6,
            Type::Blob,
            "Cached Manifest artifactType does not match its projection",
        ));
    }
    if manifest.config().digest() != &config_digest {
        return Err(cache_conversion_failure(
            6,
            Type::Blob,
            format!("Cached Manifest config digest does not match {config_digest}"),
        ));
    }
    Ok(ArtifactRefRecord {
        image_name,
        manifest_digest,
        updated_at: row.get(2)?,
        manifest_size: u64::try_from(manifest_json.len()).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(6, Type::Blob, Box::new(error))
        })?,
        manifest,
    })
}

fn cached_ref_identity_from_row(
    row: &rusqlite::Row<'_>,
    digest_column: usize,
) -> rusqlite::Result<CachedRefIdentity> {
    let (name, name_error) = identity_text_from_row(row, 0, "ref name")?;
    let (reference, reference_error) = identity_text_from_row(row, 1, "ref reference")?;
    let (manifest_digest, digest_error) =
        identity_text_from_row(row, digest_column, "manifest digest")?;
    let type_errors = [name_error, reference_error, digest_error]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    let parsed = if type_errors.is_empty() {
        ImageRef::from_repository_and_reference(&name, &reference)
            .map_err(|error| format!("Invalid Local Registry image ref: {error:#}"))
            .and_then(|image_name| {
                Digest::from_str(&manifest_digest)
                    .map(|manifest_digest| (image_name, manifest_digest))
                    .map_err(|error| format!("Invalid Local Registry manifest digest: {error}"))
            })
    } else {
        Err(type_errors.join("; "))
    };
    Ok(CachedRefIdentity {
        name,
        reference,
        manifest_digest,
        parsed,
    })
}

fn identity_text_from_row(
    row: &rusqlite::Row<'_>,
    column: usize,
    field: &str,
) -> rusqlite::Result<(String, Option<String>)> {
    use rusqlite::types::ValueRef;

    let value = row.get_ref(column)?;
    let result = match value {
        ValueRef::Text(bytes) => match std::str::from_utf8(bytes) {
            Ok(value) => (value.to_string(), None),
            Err(error) => (
                format!("<invalid UTF-8: {} bytes>", bytes.len()),
                Some(format!(
                    "Local Registry {field} is not valid UTF-8: {error}"
                )),
            ),
        },
        ValueRef::Null => (
            "<NULL>".to_string(),
            Some(format!("Local Registry {field} must be TEXT, got NULL")),
        ),
        ValueRef::Integer(value) => (
            value.to_string(),
            Some(format!("Local Registry {field} must be TEXT, got INTEGER")),
        ),
        ValueRef::Real(value) => (
            value.to_string(),
            Some(format!("Local Registry {field} must be TEXT, got REAL")),
        ),
        ValueRef::Blob(bytes) => (
            format!("<BLOB: {} bytes>", bytes.len()),
            Some(format!("Local Registry {field} must be TEXT, got BLOB")),
        ),
    };
    Ok(result)
}

fn artifact_ref_read_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<CachedRefRead<ArtifactRefRecord>> {
    let identity = cached_ref_identity_from_row(row, 3)?;
    let record = match &identity.parsed {
        Ok(_) => artifact_ref_from_row(row).map_err(|error| error.to_string()),
        Err(error) => Err(error.clone()),
    };
    Ok(CachedRefRead { identity, record })
}

fn experiment_ref_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ExperimentRefRecord> {
    let record = artifact_ref_from_row(row)?;
    let artifact_type = record
        .manifest
        .artifact_type()
        .as_ref()
        .expect("artifact_ref_from_row requires an OMMX artifactType");
    if artifact_type.as_ref() != media_types::V1_EXPERIMENT_MEDIA_TYPE {
        return Err(cache_conversion_failure(
            5,
            Type::Text,
            format!("Unexpected cached Experiment artifact type: {artifact_type}"),
        ));
    }
    if record.manifest.config().media_type().as_ref()
        != crate::experiment::EXPERIMENT_CONFIG_MEDIA_TYPE
    {
        return Err(cache_conversion_failure(
            6,
            Type::Blob,
            format!(
                "Cached Experiment config media type is {}",
                record.manifest.config().media_type()
            ),
        ));
    }
    let config_json: Vec<u8> = row.get(7)?;
    let config_digest = record.manifest.config().digest();
    if config_digest.as_ref() != sha256_digest(&config_json) {
        return Err(cache_conversion_failure(
            7,
            Type::Blob,
            format!(
                "Cached Experiment Config JSON does not match {}",
                config_digest
            ),
        ));
    }
    let config: serde_json::Value = serde_json::from_slice(&config_json)
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(7, Type::Blob, Box::new(err)))?;
    let typed_config: ExperimentConfig = serde_json::from_slice(&config_json)
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(7, Type::Blob, Box::new(err)))?;
    let run_count = u64::try_from(typed_config.runs.len())
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(7, Type::Blob, Box::new(err)))?;
    let solve_count = typed_config.runs.iter().try_fold(0_u64, |total, run| {
        let count = u64::try_from(run.solves.len()).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(7, Type::Blob, Box::new(err))
        })?;
        total.checked_add(count).ok_or_else(|| {
            rusqlite::Error::FromSqlConversionFailure(
                7,
                Type::Blob,
                std::io::Error::other("Experiment solve count overflow").into(),
            )
        })
    })?;
    let sampling_count = typed_config.runs.iter().try_fold(0_u64, |total, run| {
        let count = u64::try_from(run.samplings.len()).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(7, Type::Blob, Box::new(err))
        })?;
        total.checked_add(count).ok_or_else(|| {
            rusqlite::Error::FromSqlConversionFailure(
                7,
                Type::Blob,
                std::io::Error::other("Experiment sampling count overflow").into(),
            )
        })
    })?;
    Ok(ExperimentRefRecord {
        image_name: record.image_name,
        manifest_digest: record.manifest_digest,
        config_digest: config_digest.clone(),
        updated_at: record.updated_at,
        status: typed_config.lifecycle.status().to_string(),
        run_count,
        solve_count,
        sampling_count,
        annotations: record
            .manifest
            .annotations()
            .clone()
            .unwrap_or_default()
            .into_iter()
            .collect(),
        config,
    })
}

fn experiment_ref_read_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<CachedRefRead<ExperimentRefRecord>> {
    let identity = cached_ref_identity_from_row(row, 3)?;
    let record = match &identity.parsed {
        Ok(_) => experiment_ref_from_row(row).map_err(|error| error.to_string()),
        Err(error) => Err(error.clone()),
    };
    Ok(CachedRefRead { identity, record })
}

fn cache_conversion_failure(
    column: usize,
    column_type: Type,
    message: impl Into<String>,
) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        column,
        column_type,
        std::io::Error::new(std::io::ErrorKind::InvalidData, message.into()).into(),
    )
}

fn digest_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Digest> {
    digest_from_column(row, 0)
}

fn digest_from_column(row: &rusqlite::Row<'_>, column: usize) -> rusqlite::Result<Digest> {
    let digest: String = row.get(column)?;
    Digest::from_str(&digest)
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(column, Type::Text, Box::new(err)))
}

fn create_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS ommx_local_registry_metadata (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS refs (
            name TEXT NOT NULL,
            reference TEXT NOT NULL,
            manifest_digest TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            PRIMARY KEY(name, reference)
        );

        CREATE INDEX IF NOT EXISTS idx_refs_manifest_digest
            ON refs(manifest_digest);

        CREATE TABLE IF NOT EXISTS artifact_manifests (
            manifest_digest TEXT PRIMARY KEY,
            manifest_json BLOB NOT NULL,
            artifact_type TEXT NOT NULL,
            config_digest TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_artifact_manifests_config_digest
            ON artifact_manifests(config_digest);

        CREATE TABLE IF NOT EXISTS experiment_configs (
            config_digest TEXT PRIMARY KEY,
            config_json BLOB NOT NULL
        );
        "#,
    )?;
    Ok(())
}

fn migrate_legacy_schema_to_v2(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        DROP TABLE IF EXISTS experiment_configs;
        DROP TABLE IF EXISTS artifact_manifests;
        DROP INDEX IF EXISTS idx_refs_name;
        DROP INDEX IF EXISTS idx_refs_manifest_digest;
        ALTER TABLE refs RENAME TO refs_legacy;
        "#,
    )?;
    create_schema(conn)?;
    conn.execute_batch(
        r#"
        INSERT INTO refs (name, reference, manifest_digest, updated_at)
        SELECT name, reference, manifest_digest, updated_at
        FROM refs_legacy;
        DROP TABLE refs_legacy;
        "#,
    )?;
    Ok(())
}

fn has_current_schema(conn: &Connection) -> Result<bool> {
    Ok(table_has_columns(
        conn,
        "refs",
        &["name", "reference", "manifest_digest", "updated_at"],
    )? && table_has_columns(
        conn,
        "artifact_manifests",
        &[
            "manifest_digest",
            "manifest_json",
            "artifact_type",
            "config_digest",
        ],
    )? && table_has_columns(
        conn,
        "experiment_configs",
        &["config_digest", "config_json"],
    )?)
}

fn table_has_columns(conn: &Connection, table: &str, expected: &[&str]) -> Result<bool> {
    let mut stmt = conn.prepare("SELECT name FROM pragma_table_info(?1) ORDER BY cid")?;
    let rows = stmt.query_map([table], |row| row.get::<_, String>(0))?;
    let columns = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(columns
        .iter()
        .map(String::as_str)
        .eq(expected.iter().copied()))
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

#[cfg(test)]
fn descriptor_from_cached_manifest_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Descriptor> {
    let digest: String = row.get(0)?;
    let manifest_json: Vec<u8> = row.get(1)?;
    let digest = Digest::from_str(&digest)
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(err)))?;
    DescriptorBuilder::default()
        .media_type(MediaType::ImageManifest)
        .digest(digest)
        .size(manifest_json.len() as u64)
        .build()
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(0, Type::Text, err.into()))
}

fn random_registry_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

#[cfg(test)]
mod artifact_ref_record_tests {
    use super::*;
    use oci_spec::image::ImageManifestBuilder;

    const DIGEST: &str = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn descriptor(size: u64) -> Result<Descriptor> {
        DescriptorBuilder::default()
            .media_type(MediaType::Other("application/octet-stream".to_string()))
            .digest(Digest::from_str(DIGEST)?)
            .size(size)
            .build()
            .context("Failed to build test descriptor")
    }

    fn record(
        manifest_size: u64,
        config_size: u64,
        layer_sizes: &[u64],
    ) -> Result<ArtifactRefRecord> {
        let manifest = ImageManifestBuilder::default()
            .schema_version(2_u32)
            .config(descriptor(config_size)?)
            .layers(
                layer_sizes
                    .iter()
                    .copied()
                    .map(descriptor)
                    .collect::<Result<Vec<_>>>()?,
            )
            .build()?;
        Ok(ArtifactRefRecord {
            image_name: ImageRef::parse("example.com/ommx/size:test")?,
            manifest_digest: Digest::from_str(DIGEST)?,
            updated_at: "2026-07-11T00:00:00Z".to_string(),
            manifest_size,
            manifest,
        })
    }

    #[test]
    fn referenced_blob_size_rejects_conflicting_sizes_for_one_digest() -> Result<()> {
        let record = record(0, 1, &[2])?;
        let error = record
            .referenced_blob_size()
            .expect_err("one digest cannot declare conflicting sizes");
        assert!(error.to_string().contains("conflicting sizes"));
        Ok(())
    }

    #[test]
    fn referenced_blob_size_rejects_u64_overflow() -> Result<()> {
        let record = record(1, u64::MAX, &[])?;
        let error = record
            .referenced_blob_size()
            .expect_err("the referenced size sum must not overflow");
        assert!(error.to_string().contains("overflowed u64"));
        Ok(())
    }
}
