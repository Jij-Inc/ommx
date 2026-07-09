use super::{
    now_rfc3339, ArtifactManifestRecord, ExperimentManifestRecord, ExperimentRefRecord, RefRecord,
    RefUpdate, SQLITE_INDEX_FILE_NAME,
};
use crate::artifact::digest::validate_digest;
use crate::artifact::media_types;
use crate::artifact::{sha256_digest, ImageRef};
use anyhow::{bail, ensure, Context, Result};
#[cfg(test)]
use oci_spec::image::DescriptorBuilder;
use oci_spec::image::{Descriptor, Digest, ImageManifest, MediaType};
use rusqlite::{params, types::Type, Connection, OptionalExtension, TransactionBehavior};
use std::{
    collections::BTreeSet,
    fs,
    path::Path,
    str::FromStr,
    sync::{Mutex, MutexGuard},
    time::Duration,
};

/// SQLite Local Registry schema version stored in `PRAGMA user_version`.
/// The SQLite Local Registry has not been released yet, so incompatible
/// development schemas fail fast instead of carrying in-place migrations.
const SCHEMA_VERSION: i64 = 5;
const SQLITE_BUSY_TIMEOUT: Duration = Duration::from_secs(30);

/// SQLite-backed index store for the v3 Local Registry.
///
/// This store is the concurrency-safe equivalent of an OCI `index.json`:
/// it stores refs and their target manifest digest. Content bytes live in
/// the Local Registry content-addressed storage. The index also keeps
/// digest-addressed manifest/config projections used for blob-free catalog
/// listings; those rows are caches of CAS-addressed bytes, not a second
/// editable source of truth.
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
        tx.commit()?;
        Ok(update)
    }

    pub fn upsert_experiment_manifest(&self, experiment: &ExperimentManifestRecord) -> Result<()> {
        let mut conn = self.lock();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        Self::upsert_experiment_manifest_in(&tx, experiment)?;
        tx.commit()?;
        Ok(())
    }

    pub fn upsert_artifact_manifest(&self, artifact: &ArtifactManifestRecord) -> Result<()> {
        let mut conn = self.lock();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        Self::upsert_artifact_manifest_in(&tx, artifact)?;
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
                config_json,
                status,
                run_count,
                solve_count
            )
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(config_digest) DO UPDATE SET
                config_json = excluded.config_json,
                status = excluded.status,
                run_count = excluded.run_count,
                solve_count = excluded.solve_count
            "#,
            params![
                config_digest.to_string(),
                experiment.config_json(),
                experiment.status(),
                i64::try_from(experiment.run_count()).context("Run count does not fit in i64")?,
                i64::try_from(experiment.solve_count())
                    .context("Solve count does not fit in i64")?,
            ],
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

    pub fn list_cached_artifact_manifest_digests(
        &self,
        name_prefix: Option<&str>,
    ) -> Result<BTreeSet<String>> {
        let conn = self.lock();
        let mut out = BTreeSet::new();
        if let Some(prefix) = name_prefix {
            let prefix_len = i64::try_from(prefix.chars().count())
                .context("Ref prefix length does not fit in i64")?;
            let mut stmt = conn.prepare(
                r#"
                SELECT artifact_manifests.manifest_digest
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
                "#,
            )?;
            let rows = stmt.query_map(params![prefix_len, prefix], string_from_row)?;
            for row in rows {
                out.insert(row?);
            }
        } else {
            let mut stmt = conn.prepare(
                r#"
                SELECT artifact_manifests.manifest_digest
                FROM refs
                JOIN artifact_manifests
                  ON refs.manifest_digest = artifact_manifests.manifest_digest
                "#,
            )?;
            let rows = stmt.query_map([], string_from_row)?;
            for row in rows {
                out.insert(row?);
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

    pub fn list_invalid_experiment_config_refs(
        &self,
        name_prefix: Option<&str>,
    ) -> Result<Vec<(ImageRef, Digest)>> {
        let conn = self.lock();
        let valid_statuses = [
            crate::experiment::ExperimentStatus::Finished.as_str(),
            crate::experiment::ExperimentStatus::Draft.as_str(),
            crate::experiment::ExperimentStatus::Failed.as_str(),
            crate::experiment::ExperimentStatus::Interrupted.as_str(),
        ];
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
                JOIN experiment_configs
                  ON artifact_manifests.config_digest = experiment_configs.config_digest
                WHERE artifact_manifests.artifact_type = ?3
                  AND experiment_configs.status NOT IN (?4, ?5, ?6, ?7)
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
                params![
                    prefix_len,
                    prefix,
                    media_types::V1_EXPERIMENT_MEDIA_TYPE,
                    valid_statuses[0],
                    valid_statuses[1],
                    valid_statuses[2],
                    valid_statuses[3],
                ],
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
                JOIN experiment_configs
                  ON artifact_manifests.config_digest = experiment_configs.config_digest
                WHERE artifact_manifests.artifact_type = ?1
                  AND experiment_configs.status NOT IN (?2, ?3, ?4, ?5)
                ORDER BY refs.name, refs.reference
                "#,
            )?;
            let rows = stmt.query_map(
                params![
                    media_types::V1_EXPERIMENT_MEDIA_TYPE,
                    valid_statuses[0],
                    valid_statuses[1],
                    valid_statuses[2],
                    valid_statuses[3],
                ],
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
                    experiment_configs.status,
                    experiment_configs.run_count,
                    experiment_configs.solve_count,
                    artifact_manifests.manifest_json
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
                    experiment_configs.status,
                    experiment_configs.run_count,
                    experiment_configs.solve_count,
                    artifact_manifests.manifest_json
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
                manifest_digest TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                PRIMARY KEY(name, reference)
            );

            CREATE INDEX IF NOT EXISTS idx_refs_name ON refs(name);

            CREATE TABLE IF NOT EXISTS artifact_manifests (
                manifest_digest TEXT PRIMARY KEY,
                manifest_json BLOB NOT NULL,
                artifact_type TEXT NOT NULL,
                config_digest TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_artifact_manifests_artifact_type
                ON artifact_manifests(artifact_type);

            CREATE TABLE IF NOT EXISTS experiment_configs (
                config_digest TEXT PRIMARY KEY,
                config_json BLOB NOT NULL,
                status TEXT NOT NULL,
                run_count INTEGER NOT NULL CHECK(run_count >= 0),
                solve_count INTEGER NOT NULL CHECK(solve_count >= 0)
            );

            CREATE INDEX IF NOT EXISTS idx_experiment_configs_status
                ON experiment_configs(status);
            "#,
        )?;
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

fn experiment_ref_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ExperimentRefRecord> {
    let name: String = row.get(0)?;
    let reference: String = row.get(1)?;
    let image_name = ImageRef::from_repository_and_reference(&name, &reference)
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(0, Type::Text, err.into()))?;
    let digest: String = row.get(3)?;
    let manifest_digest = Digest::from_str(&digest)
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(3, Type::Text, Box::new(err)))?;
    let status: String = row.get(4)?;
    crate::experiment::ExperimentStatus::from_config(&status)
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(4, Type::Text, err.into()))?;
    let manifest_json: Vec<u8> = row.get(7)?;
    let manifest: ImageManifest = serde_json::from_slice(&manifest_json)
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(7, Type::Blob, Box::new(err)))?;
    let annotations = manifest
        .annotations()
        .clone()
        .unwrap_or_default()
        .into_iter()
        .collect();
    Ok(ExperimentRefRecord {
        image_name,
        manifest_digest,
        updated_at: row.get(2)?,
        status,
        run_count: read_u64(row, 5)?,
        solve_count: read_u64(row, 6)?,
        annotations,
    })
}

fn string_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<String> {
    row.get(0)
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

fn read_u64(row: &rusqlite::Row<'_>, idx: usize) -> rusqlite::Result<u64> {
    let value: i64 = row.get(idx)?;
    u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(idx, value))
}

fn random_registry_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}
