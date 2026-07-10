use super::{
    now_rfc3339, ArtifactManifestRecord, ArtifactRefRecord, ExperimentManifestRecord,
    ExperimentRefRecord, RefRecord, RefUpdate, SQLITE_INDEX_FILE_NAME,
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
    fs,
    path::Path,
    str::FromStr,
    sync::{Mutex, MutexGuard},
    time::Duration,
};

/// SQLite Local Registry schema version stored in `PRAGMA user_version`.
const SCHEMA_VERSION: i64 = 2;
const SQLITE_BUSY_TIMEOUT: Duration = Duration::from_secs(30);

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
        Self::prune_unreferenced_cache_in(&tx)?;
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

    pub fn list_missing_artifact_manifest_digests(
        &self,
        name_prefix: Option<&str>,
    ) -> Result<Vec<Digest>> {
        let conn = self.lock();
        let mut out = Vec::new();
        if let Some(prefix) = name_prefix {
            let prefix_len = i64::try_from(prefix.chars().count())
                .context("Ref prefix length does not fit in i64")?;
            let mut stmt = conn.prepare(
                r#"
                SELECT DISTINCT refs.manifest_digest
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
                ORDER BY refs.manifest_digest
                "#,
            )?;
            let rows = stmt.query_map(params![prefix_len, prefix], digest_from_row)?;
            for row in rows {
                out.push(row?);
            }
        } else {
            let mut stmt = conn.prepare(
                r#"
                SELECT DISTINCT refs.manifest_digest
                FROM refs
                LEFT JOIN artifact_manifests
                  ON refs.manifest_digest = artifact_manifests.manifest_digest
                WHERE artifact_manifests.manifest_digest IS NULL
                ORDER BY refs.manifest_digest
                "#,
            )?;
            let rows = stmt.query_map([], digest_from_row)?;
            for row in rows {
                out.push(row?);
            }
        }
        Ok(out)
    }

    pub fn list_artifact_refs(&self, name_prefix: Option<&str>) -> Result<Vec<ArtifactRefRecord>> {
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
            let rows = stmt.query_map(params![prefix_len, prefix], artifact_ref_from_row)?;
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
            let rows = stmt.query_map([], artifact_ref_from_row)?;
            for row in rows {
                out.push(row?);
            }
        }
        Ok(out)
    }

    pub fn list_missing_experiment_config_refs(
        &self,
        name_prefix: Option<&str>,
    ) -> Result<Vec<(ImageRef, Digest)>> {
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
                image_ref_and_digest_from_row,
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
            let rows = stmt.query_map(
                params![media_types::V1_EXPERIMENT_MEDIA_TYPE],
                image_ref_and_digest_from_row,
            )?;
            for row in rows {
                out.push(row?);
            }
        }
        Ok(out)
    }

    pub fn list_experiment_refs(
        &self,
        name_prefix: Option<&str>,
    ) -> Result<Vec<ExperimentRefRecord>> {
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
                experiment_ref_from_row,
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
                experiment_ref_from_row,
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

struct CachedArtifactRef {
    record: ArtifactRefRecord,
    manifest: ImageManifest,
}

fn cached_artifact_ref_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CachedArtifactRef> {
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
    let manifest_value: serde_json::Value = serde_json::from_slice(&manifest_json)
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(6, Type::Blob, Box::new(err)))?;
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
    let annotations = manifest
        .annotations()
        .clone()
        .unwrap_or_default()
        .into_iter()
        .collect();
    Ok(CachedArtifactRef {
        record: ArtifactRefRecord {
            image_name,
            manifest_digest,
            updated_at: row.get(2)?,
            artifact_type,
            config_digest,
            annotations,
            manifest: manifest_value,
        },
        manifest,
    })
}

fn artifact_ref_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ArtifactRefRecord> {
    Ok(cached_artifact_ref_from_row(row)?.record)
}

fn experiment_ref_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ExperimentRefRecord> {
    let CachedArtifactRef { record, manifest } = cached_artifact_ref_from_row(row)?;
    if record.artifact_type.as_ref() != media_types::V1_EXPERIMENT_MEDIA_TYPE {
        return Err(cache_conversion_failure(
            5,
            Type::Text,
            format!(
                "Unexpected cached Experiment artifact type: {}",
                record.artifact_type
            ),
        ));
    }
    if manifest.config().media_type().as_ref() != crate::experiment::EXPERIMENT_CONFIG_MEDIA_TYPE {
        return Err(cache_conversion_failure(
            6,
            Type::Blob,
            format!(
                "Cached Experiment config media type is {}",
                manifest.config().media_type()
            ),
        ));
    }
    let config_json: Vec<u8> = row.get(7)?;
    if record.config_digest.as_ref() != sha256_digest(&config_json) {
        return Err(cache_conversion_failure(
            7,
            Type::Blob,
            format!(
                "Cached Experiment Config JSON does not match {}",
                record.config_digest
            ),
        ));
    }
    let config: serde_json::Value = serde_json::from_slice(&config_json)
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(7, Type::Blob, Box::new(err)))?;
    let typed_config: ExperimentConfig = serde_json::from_slice(&config_json)
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(7, Type::Blob, Box::new(err)))?;
    crate::experiment::ExperimentStatus::from_config(&typed_config.status)
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(7, Type::Blob, err.into()))?;
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
    Ok(ExperimentRefRecord {
        image_name: record.image_name,
        manifest_digest: record.manifest_digest,
        config_digest: record.config_digest,
        updated_at: record.updated_at,
        status: typed_config.status,
        run_count,
        solve_count,
        annotations: record.annotations,
        config,
    })
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
    let digest: String = row.get(0)?;
    Digest::from_str(&digest)
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(err)))
}

fn image_ref_and_digest_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<(ImageRef, Digest)> {
    let name: String = row.get(0)?;
    let reference: String = row.get(1)?;
    let image_name = ImageRef::from_repository_and_reference(&name, &reference)
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(0, Type::Text, err.into()))?;
    let digest: String = row.get(2)?;
    let digest = Digest::from_str(&digest)
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(2, Type::Text, Box::new(err)))?;
    Ok((image_name, digest))
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
