//! SQLite-backed local registry index and filesystem content store.
//!
//! This module is intentionally independent from the current `ocipkg::OciDir`
//! local-registry layout. The legacy layout remains a read/import source; new
//! local-registry state is represented by an index store plus a CAS blob store.

use anyhow::{ensure, Context, Result};
use chrono::Utc;
use ocipkg::{
    oci_spec::image::{Descriptor, ImageIndex, ImageManifest, OciLayout},
    ImageName,
};
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest as _, Sha256};
use std::{
    fs,
    fs::OpenOptions,
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
    time::Duration,
};
use uuid::Uuid;

pub const SQLITE_INDEX_FILE_NAME: &str = "index.sqlite3";
pub const FILE_BLOB_STORE_DIR_NAME: &str = "blobs";
pub const OCI_IMAGE_REF_NAME_ANNOTATION: &str = "org.opencontainers.image.ref.name";

pub const BLOB_KIND_BLOB: &str = "blob";
pub const BLOB_KIND_CONFIG: &str = "config";
pub const BLOB_KIND_LAYER: &str = "layer";
pub const BLOB_KIND_MANIFEST: &str = "manifest";

const SCHEMA_VERSION: i64 = 1;
const SQLITE_BUSY_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobRecord {
    pub digest: String,
    pub size: u64,
    pub media_type: Option<String>,
    pub storage_uri: String,
    pub kind: String,
    pub last_verified_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestRecord {
    pub digest: String,
    pub media_type: String,
    pub size: u64,
    pub subject_digest: Option<String>,
    pub annotations_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefRecord {
    pub name: String,
    pub reference: String,
    pub manifest_digest: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerRecord {
    pub manifest_digest: String,
    pub position: u32,
    pub digest: String,
    pub media_type: String,
    pub size: u64,
    pub annotations_json: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefConflictPolicy {
    KeepExisting,
    Replace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefUpdate {
    Inserted,
    Unchanged,
    Replaced {
        previous_manifest_digest: String,
    },
    Conflicted {
        existing_manifest_digest: String,
        incoming_manifest_digest: String,
    },
}

#[derive(Debug)]
pub struct SqliteIndexStore {
    conn: Connection,
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
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    pub fn open_in_registry_root(root: impl AsRef<Path>) -> Result<Self> {
        Self::open(root.as_ref().join(SQLITE_INDEX_FILE_NAME))
    }

    pub fn schema_version(&self) -> Result<i64> {
        self.conn
            .query_row(
                "SELECT version FROM ommx_local_registry_schema LIMIT 1",
                [],
                |row| row.get(0),
            )
            .context("Failed to read local registry schema version")
    }

    pub fn put_blob(&self, record: &BlobRecord) -> Result<()> {
        validate_digest(&record.digest)?;
        if let Some(existing) = self.get_blob(&record.digest)? {
            ensure!(
                existing.size == record.size,
                "Blob size mismatch for digest {}: existing={}, new={}",
                record.digest,
                existing.size,
                record.size
            );
        }
        let now = now_rfc3339();
        self.conn.execute(
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
        self.conn
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
        validate_digest(&record.digest)?;
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
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
        tx.execute(
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
            tx.execute(
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
        tx.commit()?;
        Ok(())
    }

    pub fn get_manifest(&self, digest: &str) -> Result<Option<ManifestRecord>> {
        validate_digest(digest)?;
        self.conn
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
        let mut stmt = self.conn.prepare(
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
        validate_digest(manifest_digest)?;
        self.conn.execute(
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
        validate_digest(manifest_digest)?;
        if policy == RefConflictPolicy::Replace {
            return self.replace_ref(name, reference, manifest_digest);
        }

        let inserted = self.conn.execute(
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

        let existing_manifest_digest = self.resolve_ref(name, reference)?.with_context(|| {
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

    fn replace_ref(&self, name: &str, reference: &str, manifest_digest: &str) -> Result<RefUpdate> {
        let previous_manifest_digest = self.resolve_ref(name, reference)?;
        if previous_manifest_digest.as_deref() == Some(manifest_digest) {
            return Ok(RefUpdate::Unchanged);
        }

        self.put_ref(name, reference, manifest_digest)?;
        Ok(match previous_manifest_digest {
            Some(previous_manifest_digest) => RefUpdate::Replaced {
                previous_manifest_digest,
            },
            None => RefUpdate::Inserted,
        })
    }

    pub fn resolve_ref(&self, name: &str, reference: &str) -> Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT manifest_digest FROM refs WHERE name = ?1 AND reference = ?2",
                params![name, reference],
                |row| row.get(0),
            )
            .optional()
            .context("Failed to resolve local registry ref")
    }

    pub fn list_refs(&self, name_prefix: Option<&str>) -> Result<Vec<RefRecord>> {
        let mut out = Vec::new();
        if let Some(prefix) = name_prefix {
            let like = format!("{prefix}%");
            let mut stmt = self.conn.prepare(
                r#"
                SELECT name, reference, manifest_digest, updated_at
                FROM refs
                WHERE name LIKE ?1
                ORDER BY name, reference
                "#,
            )?;
            let rows = stmt.query_map(params![like], ref_from_row)?;
            for row in rows {
                out.push(row?);
            }
        } else {
            let mut stmt = self.conn.prepare(
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
        self.conn.execute_batch(
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

#[derive(Debug, Clone)]
pub struct FileBlobStore {
    root: PathBuf,
}

impl FileBlobStore {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        fs::create_dir_all(&root)
            .with_context(|| format!("Failed to create blob store {}", root.display()))?;
        Ok(Self { root })
    }

    pub fn open_in_registry_root(root: impl AsRef<Path>) -> Result<Self> {
        Self::open(root.as_ref().join(FILE_BLOB_STORE_DIR_NAME))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn put_bytes(&self, bytes: &[u8]) -> Result<BlobRecord> {
        let digest = sha256_digest(bytes);
        let path = self.path_for_digest(&digest)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }
        if path.exists() {
            verify_existing_blob(&path, bytes, &digest)?;
        } else {
            self.write_blob_atomically(bytes, &digest, &path)?;
        }
        Ok(BlobRecord {
            digest,
            size: bytes.len() as u64,
            media_type: None,
            storage_uri: path.to_string_lossy().into_owned(),
            kind: BLOB_KIND_BLOB.to_string(),
            last_verified_at: Some(now_rfc3339()),
        })
    }

    pub fn read_bytes(&self, digest: &str) -> Result<Vec<u8>> {
        let path = self.path_for_digest(digest)?;
        let bytes =
            fs::read(&path).with_context(|| format!("Failed to read blob {}", path.display()))?;
        ensure!(
            sha256_digest(&bytes) == digest,
            "Blob digest verification failed for {digest}"
        );
        Ok(bytes)
    }

    pub fn exists(&self, digest: &str) -> Result<bool> {
        Ok(self.path_for_digest(digest)?.exists())
    }

    pub fn path_for_digest(&self, digest: &str) -> Result<PathBuf> {
        let (algorithm, encoded) = split_digest(digest)?;
        Ok(self.root.join(algorithm).join(encoded))
    }

    fn write_blob_atomically(&self, bytes: &[u8], digest: &str, path: &Path) -> Result<()> {
        let temp_path = self.temp_path_for_digest(digest)?;
        let mut temp_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .with_context(|| format!("Failed to create temporary blob {}", temp_path.display()))?;
        temp_file
            .write_all(bytes)
            .with_context(|| format!("Failed to write temporary blob {}", temp_path.display()))?;
        temp_file
            .sync_all()
            .with_context(|| format!("Failed to sync temporary blob {}", temp_path.display()))?;
        drop(temp_file);

        match fs::hard_link(&temp_path, path) {
            Ok(()) => {
                let _ = fs::remove_file(&temp_path);
                Ok(())
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                let _ = fs::remove_file(&temp_path);
                verify_existing_blob(path, bytes, digest)
            }
            Err(error) => {
                let _ = fs::remove_file(&temp_path);
                Err(error).with_context(|| {
                    format!(
                        "Failed to publish blob {} from {} to {}",
                        digest,
                        temp_path.display(),
                        path.display()
                    )
                })
            }
        }
    }

    fn temp_path_for_digest(&self, digest: &str) -> Result<PathBuf> {
        let path = self.path_for_digest(digest)?;
        let encoded = path
            .file_name()
            .and_then(|name| name.to_str())
            .context("Blob digest path has no file name")?;
        Ok(path.with_file_name(format!(".{encoded}.{}.tmp", Uuid::new_v4())))
    }
}

#[derive(Debug)]
pub struct LocalRegistry {
    root: PathBuf,
    index: SqliteIndexStore,
    blobs: FileBlobStore,
}

impl LocalRegistry {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        let index = SqliteIndexStore::open_in_registry_root(&root)?;
        let blobs = FileBlobStore::open_in_registry_root(&root)?;
        Ok(Self { root, index, blobs })
    }

    pub fn open_default() -> Result<Self> {
        Self::open(super::get_local_registry_root())
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn index(&self) -> &SqliteIndexStore {
        &self.index
    }

    pub fn blobs(&self) -> &FileBlobStore {
        &self.blobs
    }

    pub fn import_legacy_ref(&self, image_name: &ImageName) -> Result<LegacyOciDirImport> {
        import_legacy_local_registry_ref(&self.index, &self.blobs, &self.root, image_name)
    }

    pub fn migrate_legacy_layout(&self) -> Result<LegacyMigrationReport> {
        migrate_legacy_local_registry(&self.index, &self.blobs, &self.root)
    }

    pub fn migrate_legacy_layout_with_policy(
        &self,
        policy: RefConflictPolicy,
    ) -> Result<LegacyMigrationReport> {
        migrate_legacy_local_registry_with_policy(&self.index, &self.blobs, &self.root, policy)
    }

    pub fn resolve_image_name(&self, image_name: &ImageName) -> Result<Option<String>> {
        self.index.resolve_image_name(image_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyOciDirImport {
    pub manifest_digest: String,
    pub image_name: Option<ImageName>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyOciDirRef {
    pub manifest_digest: String,
    pub image_name: Option<ImageName>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyMigrationReport {
    pub scanned_dirs: usize,
    pub imported_dirs: usize,
    pub verified_dirs: usize,
    pub conflicted_dirs: usize,
    pub replaced_refs: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RefConflictHandling {
    Error,
    Return,
}

/// Import an existing OCI Image Layout directory into the v3 local registry.
///
/// This is the compatibility path for the current OMMX local registry layout:
/// each path/tag entry is a standalone OCI directory with `oci-layout`,
/// `index.json`, and `blobs/`. The v3 registry does not keep using that
/// `index.json` as mutable state; it only reads it to discover the manifest and
/// then copies the exact content-addressed blobs into [`FileBlobStore`].
pub fn import_legacy_oci_dir(
    index_store: &SqliteIndexStore,
    blob_store: &FileBlobStore,
    oci_dir_root: impl AsRef<Path>,
) -> Result<LegacyOciDirImport> {
    import_legacy_oci_dir_with_policy(
        index_store,
        blob_store,
        oci_dir_root,
        RefConflictPolicy::KeepExisting,
    )
}

pub fn import_legacy_oci_dir_with_policy(
    index_store: &SqliteIndexStore,
    blob_store: &FileBlobStore,
    oci_dir_root: impl AsRef<Path>,
    policy: RefConflictPolicy,
) -> Result<LegacyOciDirImport> {
    let (import, _) = import_legacy_oci_dir_with_policy_inner(
        index_store,
        blob_store,
        oci_dir_root,
        policy,
        RefConflictHandling::Error,
    )?;
    Ok(import)
}

fn import_legacy_oci_dir_with_policy_inner(
    index_store: &SqliteIndexStore,
    blob_store: &FileBlobStore,
    oci_dir_root: impl AsRef<Path>,
    policy: RefConflictPolicy,
    conflict_handling: RefConflictHandling,
) -> Result<(LegacyOciDirImport, Option<RefUpdate>)> {
    let oci_dir_root = oci_dir_root.as_ref();
    ensure_legacy_oci_layout(oci_dir_root)?;

    let index_path = oci_dir_root.join("index.json");
    let image_index: ImageIndex = read_json_file(&index_path)?;
    ensure!(
        image_index.manifests().len() == 1,
        "Legacy OMMX local registry entry must contain exactly one manifest: {}",
        index_path.display()
    );
    let manifest_desc = image_index.manifests().first().unwrap();
    let image_name = image_name_from_index_descriptor(manifest_desc)?;
    let manifest_digest = digest_to_string(manifest_desc.digest());
    if conflict_handling == RefConflictHandling::Error {
        if let Some(image_name) = &image_name {
            ensure_image_ref_update_allowed(index_store, image_name, &manifest_digest, policy)?;
        }
    }

    put_descriptor_blob(
        index_store,
        blob_store,
        oci_dir_root,
        manifest_desc,
        BLOB_KIND_MANIFEST,
    )?;

    let manifest_bytes = blob_store.read_bytes(&manifest_digest)?;
    let manifest: ImageManifest = serde_json::from_slice(&manifest_bytes)
        .with_context(|| format!("Failed to parse legacy manifest {manifest_digest}"))?;

    put_descriptor_blob(
        index_store,
        blob_store,
        oci_dir_root,
        manifest.config(),
        BLOB_KIND_CONFIG,
    )?;

    let mut layers = Vec::with_capacity(manifest.layers().len());
    for (position, layer) in manifest.layers().iter().enumerate() {
        put_descriptor_blob(
            index_store,
            blob_store,
            oci_dir_root,
            layer,
            BLOB_KIND_LAYER,
        )?;
        layers.push(LayerRecord {
            manifest_digest: manifest_digest.clone(),
            position: u32::try_from(position).context("Layer position does not fit in u32")?,
            digest: digest_to_string(layer.digest()),
            media_type: layer.media_type().to_string(),
            size: layer.size(),
            annotations_json: annotations_json(layer.annotations())?,
        });
    }

    index_store.put_manifest(
        &ManifestRecord {
            digest: manifest_digest.clone(),
            media_type: manifest_desc.media_type().to_string(),
            size: manifest_desc.size(),
            subject_digest: manifest
                .subject()
                .as_ref()
                .map(|d| digest_to_string(d.digest())),
            annotations_json: annotations_json(manifest.annotations())?,
            created_at: now_rfc3339(),
        },
        &layers,
    )?;

    let ref_update = image_name
        .as_ref()
        .map(|image_name| {
            put_image_ref_with_conflict_handling(
                index_store,
                image_name,
                &manifest_digest,
                policy,
                conflict_handling,
            )
        })
        .transpose()?;

    Ok((
        LegacyOciDirImport {
            manifest_digest,
            image_name,
        },
        ref_update,
    ))
}

pub fn import_legacy_local_registry_ref(
    index_store: &SqliteIndexStore,
    blob_store: &FileBlobStore,
    legacy_registry_root: impl AsRef<Path>,
    image_name: &ImageName,
) -> Result<LegacyOciDirImport> {
    let legacy_path = legacy_local_registry_path(legacy_registry_root, image_name);
    import_legacy_oci_dir_as_ref_with_policy(
        index_store,
        blob_store,
        legacy_path,
        image_name,
        RefConflictPolicy::KeepExisting,
    )
}

pub fn import_legacy_oci_dir_as_ref(
    index_store: &SqliteIndexStore,
    blob_store: &FileBlobStore,
    oci_dir_root: impl AsRef<Path>,
    image_name: &ImageName,
) -> Result<LegacyOciDirImport> {
    import_legacy_oci_dir_as_ref_with_policy(
        index_store,
        blob_store,
        oci_dir_root,
        image_name,
        RefConflictPolicy::KeepExisting,
    )
}

pub fn import_legacy_oci_dir_as_ref_with_policy(
    index_store: &SqliteIndexStore,
    blob_store: &FileBlobStore,
    oci_dir_root: impl AsRef<Path>,
    image_name: &ImageName,
    policy: RefConflictPolicy,
) -> Result<LegacyOciDirImport> {
    let (import, _) = import_legacy_oci_dir_as_ref_with_policy_inner(
        index_store,
        blob_store,
        oci_dir_root,
        image_name,
        policy,
        RefConflictHandling::Error,
    )?;
    Ok(import)
}

fn import_legacy_oci_dir_as_ref_with_policy_inner(
    index_store: &SqliteIndexStore,
    blob_store: &FileBlobStore,
    oci_dir_root: impl AsRef<Path>,
    image_name: &ImageName,
    policy: RefConflictPolicy,
    conflict_handling: RefConflictHandling,
) -> Result<(LegacyOciDirImport, RefUpdate)> {
    let legacy_path = oci_dir_root.as_ref();
    let legacy_ref = legacy_oci_dir_ref(legacy_path)?;
    if let Some(imported_name) = &legacy_ref.image_name {
        ensure!(
            imported_name == image_name,
            "Legacy local registry ref mismatch: requested={}, imported={}",
            image_name,
            imported_name
        );
    }

    if conflict_handling == RefConflictHandling::Error {
        ensure_image_ref_update_allowed(
            index_store,
            image_name,
            &legacy_ref.manifest_digest,
            policy,
        )?;
    }
    let (import, annotation_update) = import_legacy_oci_dir_with_policy_inner(
        index_store,
        blob_store,
        legacy_path,
        policy,
        conflict_handling,
    )?;
    let ref_update = match annotation_update {
        Some(update) => update,
        None => put_image_ref_with_conflict_handling(
            index_store,
            image_name,
            &import.manifest_digest,
            policy,
            conflict_handling,
        )?,
    };
    Ok((import, ref_update))
}

pub fn migrate_legacy_local_registry(
    index_store: &SqliteIndexStore,
    blob_store: &FileBlobStore,
    legacy_registry_root: impl AsRef<Path>,
) -> Result<LegacyMigrationReport> {
    migrate_legacy_local_registry_with_policy(
        index_store,
        blob_store,
        legacy_registry_root,
        RefConflictPolicy::KeepExisting,
    )
}

pub fn migrate_legacy_local_registry_with_policy(
    index_store: &SqliteIndexStore,
    blob_store: &FileBlobStore,
    legacy_registry_root: impl AsRef<Path>,
    policy: RefConflictPolicy,
) -> Result<LegacyMigrationReport> {
    let legacy_registry_root = legacy_registry_root.as_ref();
    let legacy_dirs = gather_legacy_oci_dirs(legacy_registry_root)?;
    let mut report = LegacyMigrationReport {
        scanned_dirs: legacy_dirs.len(),
        imported_dirs: 0,
        verified_dirs: 0,
        conflicted_dirs: 0,
        replaced_refs: 0,
    };

    for legacy_dir in &legacy_dirs {
        let image_name = legacy_migration_image_name(legacy_registry_root, legacy_dir)?;
        let legacy_ref = legacy_oci_dir_ref(legacy_dir)?;
        let existing_manifest_digest = index_store.resolve_image_name(&image_name)?;

        match existing_manifest_digest {
            None => {
                let (_, ref_update) = import_legacy_oci_dir_as_ref_with_policy_inner(
                    index_store,
                    blob_store,
                    legacy_dir,
                    &image_name,
                    RefConflictPolicy::KeepExisting,
                    RefConflictHandling::Return,
                )
                .with_context(|| {
                    format!(
                        "Failed to migrate legacy local registry entry {}",
                        legacy_dir.display()
                    )
                })?;
                record_migration_ref_update(&mut report, ref_update);
            }
            Some(existing) if existing == legacy_ref.manifest_digest => {
                let (_, ref_update) = import_legacy_oci_dir_as_ref_with_policy_inner(
                    index_store,
                    blob_store,
                    legacy_dir,
                    &image_name,
                    RefConflictPolicy::KeepExisting,
                    RefConflictHandling::Return,
                )
                .with_context(|| {
                    format!(
                        "Failed to verify migrated legacy local registry entry {}",
                        legacy_dir.display()
                    )
                })?;
                record_migration_ref_update(&mut report, ref_update);
            }
            Some(_) if policy == RefConflictPolicy::KeepExisting => {
                report.conflicted_dirs += 1;
            }
            Some(_) => {
                let (_, ref_update) = import_legacy_oci_dir_as_ref_with_policy_inner(
                    index_store,
                    blob_store,
                    legacy_dir,
                    &image_name,
                    RefConflictPolicy::Replace,
                    RefConflictHandling::Return,
                )
                .with_context(|| {
                    format!(
                        "Failed to replace legacy local registry entry {}",
                        legacy_dir.display()
                    )
                })?;
                record_migration_ref_update(&mut report, ref_update);
            }
        }
    }

    Ok(report)
}

pub fn legacy_oci_dir_image_name(oci_dir_root: impl AsRef<Path>) -> Result<Option<ImageName>> {
    Ok(legacy_oci_dir_ref(oci_dir_root)?.image_name)
}

pub fn legacy_oci_dir_ref(oci_dir_root: impl AsRef<Path>) -> Result<LegacyOciDirRef> {
    let oci_dir_root = oci_dir_root.as_ref();
    ensure_legacy_oci_layout(oci_dir_root)?;

    let index_path = oci_dir_root.join("index.json");
    let image_index: ImageIndex = read_json_file(&index_path)?;
    ensure!(
        image_index.manifests().len() == 1,
        "Legacy OMMX local registry entry must contain exactly one manifest: {}",
        index_path.display()
    );
    let manifest_desc = image_index.manifests().first().unwrap();
    Ok(LegacyOciDirRef {
        manifest_digest: digest_to_string(manifest_desc.digest()),
        image_name: image_name_from_index_descriptor(manifest_desc)?,
    })
}

pub fn legacy_local_registry_path(
    legacy_registry_root: impl AsRef<Path>,
    image_name: &ImageName,
) -> PathBuf {
    legacy_registry_root.as_ref().join(image_name.as_path())
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

pub fn image_name_repository(image_name: &ImageName) -> String {
    if let Some(port) = image_name.port {
        format!("{}:{}/{}", image_name.hostname, port, image_name.name)
    } else {
        format!("{}/{}", image_name.hostname, image_name.name)
    }
}

fn gather_legacy_oci_dirs(root: &Path) -> Result<Vec<PathBuf>> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut dirs = Vec::new();
    gather_legacy_oci_dirs_inner(root, &mut dirs)?;
    Ok(dirs)
}

fn gather_legacy_oci_dirs_inner(dir: &Path, dirs: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("Failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if path.join("oci-layout").exists() {
            dirs.push(path);
        } else {
            gather_legacy_oci_dirs_inner(&path, dirs)?;
        }
    }
    Ok(())
}

fn legacy_migration_image_name(
    legacy_registry_root: &Path,
    legacy_dir: &Path,
) -> Result<ImageName> {
    let annotated = legacy_oci_dir_image_name(legacy_dir)?;
    let path_name = legacy_dir
        .strip_prefix(legacy_registry_root)
        .ok()
        .and_then(|relative| ImageName::from_path(relative).ok());

    match (annotated, path_name) {
        (Some(annotated), Some(path_name)) => {
            ensure!(
                annotated == path_name,
                "Legacy local registry ref mismatch: path={}, annotation={}",
                path_name,
                annotated
            );
            Ok(annotated)
        }
        (Some(annotated), None) => Ok(annotated),
        (None, Some(path_name)) => Ok(path_name),
        (None, None) => {
            anyhow::bail!(
                "Cannot infer image name for legacy local registry entry {}",
                legacy_dir.display()
            )
        }
    }
}

fn ensure_image_ref_update_allowed(
    index_store: &SqliteIndexStore,
    image_name: &ImageName,
    manifest_digest: &str,
    policy: RefConflictPolicy,
) -> Result<()> {
    if policy == RefConflictPolicy::Replace {
        return Ok(());
    }

    if let Some(existing_manifest_digest) = index_store.resolve_image_name(image_name)? {
        ensure!(
            existing_manifest_digest == manifest_digest,
            "Local registry ref conflict for {}: existing manifest {}, incoming manifest {}",
            image_name,
            existing_manifest_digest,
            manifest_digest
        );
    }
    Ok(())
}

fn put_image_ref_with_conflict_handling(
    index_store: &SqliteIndexStore,
    image_name: &ImageName,
    manifest_digest: &str,
    policy: RefConflictPolicy,
    conflict_handling: RefConflictHandling,
) -> Result<RefUpdate> {
    match index_store.put_image_ref_with_policy(image_name, manifest_digest, policy)? {
        RefUpdate::Conflicted {
            existing_manifest_digest,
            incoming_manifest_digest,
        } if conflict_handling == RefConflictHandling::Error => {
            anyhow::bail!(
                "Local registry ref conflict for {}: existing manifest {}, incoming manifest {}",
                image_name,
                existing_manifest_digest,
                incoming_manifest_digest
            )
        }
        RefUpdate::Conflicted {
            existing_manifest_digest,
            incoming_manifest_digest,
        } => Ok(RefUpdate::Conflicted {
            existing_manifest_digest,
            incoming_manifest_digest,
        }),
        update => Ok(update),
    }
}

fn record_migration_ref_update(report: &mut LegacyMigrationReport, update: RefUpdate) {
    match update {
        RefUpdate::Inserted => report.imported_dirs += 1,
        RefUpdate::Unchanged => report.verified_dirs += 1,
        RefUpdate::Replaced { .. } => report.replaced_refs += 1,
        RefUpdate::Conflicted { .. } => report.conflicted_dirs += 1,
    }
}

fn verify_existing_blob(path: &Path, bytes: &[u8], digest: &str) -> Result<()> {
    let existing = fs::read(path)
        .with_context(|| format!("Failed to read existing blob {}", path.display()))?;
    ensure!(
        existing == bytes,
        "Existing blob has different bytes for digest {digest}"
    );
    Ok(())
}

pub fn sha256_digest(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("sha256:{}", encode_hex(&digest))
}

fn ensure_legacy_oci_layout(oci_dir_root: &Path) -> Result<()> {
    let layout_path = oci_dir_root.join("oci-layout");
    let layout: OciLayout = read_json_file(&layout_path)?;
    ensure!(
        layout.image_layout_version() == "1.0.0",
        "Unsupported OCI layout version in {}: {}",
        layout_path.display(),
        layout.image_layout_version()
    );
    Ok(())
}

fn put_descriptor_blob(
    index_store: &SqliteIndexStore,
    blob_store: &FileBlobStore,
    oci_dir_root: &Path,
    desc: &Descriptor,
    kind: &str,
) -> Result<()> {
    let digest = digest_to_string(desc.digest());
    let bytes = read_legacy_blob(oci_dir_root, &digest)
        .with_context(|| format!("Failed to read legacy {kind} blob {digest}"))?;
    ensure!(
        bytes.len() as u64 == desc.size(),
        "Legacy {kind} blob size mismatch for {digest}: descriptor={}, actual={}",
        desc.size(),
        bytes.len()
    );

    let mut record = blob_store.put_bytes(&bytes)?;
    ensure!(
        record.digest == digest,
        "Legacy {kind} blob digest mismatch: descriptor={}, actual={}",
        digest,
        record.digest
    );
    record.media_type = Some(desc.media_type().to_string());
    record.kind = kind.to_string();
    index_store.put_blob(&record)
}

fn read_legacy_blob(oci_dir_root: &Path, digest: &str) -> Result<Vec<u8>> {
    let path = legacy_blob_path(oci_dir_root, digest)?;
    let bytes = fs::read(&path).with_context(|| format!("Failed to read {}", path.display()))?;
    ensure!(
        sha256_digest(&bytes) == digest,
        "Legacy blob digest verification failed for {digest}"
    );
    Ok(bytes)
}

fn legacy_blob_path(oci_dir_root: &Path, digest: &str) -> Result<PathBuf> {
    let (algorithm, encoded) = split_digest(digest)?;
    Ok(oci_dir_root.join("blobs").join(algorithm).join(encoded))
}

fn read_json_file<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let bytes = fs::read(path).with_context(|| format!("Failed to read {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("Failed to parse {}", path.display()))
}

fn image_name_from_index_descriptor(desc: &Descriptor) -> Result<Option<ImageName>> {
    desc.annotations()
        .as_ref()
        .and_then(|annotations| annotations.get(OCI_IMAGE_REF_NAME_ANNOTATION))
        .map(|name| ImageName::parse(name).with_context(|| format!("Invalid image ref: {name}")))
        .transpose()
}

fn digest_to_string(digest: &ocipkg::Digest) -> String {
    digest.to_string()
}

fn annotations_json(
    annotations: &Option<std::collections::HashMap<String, String>>,
) -> Result<String> {
    match annotations {
        Some(annotations) => {
            serde_json::to_string(annotations).context("Failed to encode annotations")
        }
        None => Ok("{}".to_string()),
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

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

fn validate_digest(digest: &str) -> Result<()> {
    let (algorithm, encoded) = split_digest(digest)?;
    ensure!(
        algorithm == "sha256",
        "Unsupported digest algorithm: {algorithm}"
    );
    ensure!(
        encoded.len() == 64 && encoded.bytes().all(|b| b.is_ascii_hexdigit()),
        "Invalid sha256 digest: {digest}"
    );
    Ok(())
}

fn split_digest(digest: &str) -> Result<(&str, &str)> {
    let (algorithm, encoded) = digest
        .split_once(':')
        .with_context(|| format!("Digest must be '<algorithm>:<encoded>': {digest}"))?;
    ensure!(!algorithm.is_empty(), "Digest algorithm is empty");
    ensure!(!encoded.is_empty(), "Digest value is empty");
    Ok((algorithm, encoded))
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::media_types;
    use ocipkg::{
        image::{ImageBuilder, OciDirBuilder},
        oci_spec::image::{DescriptorBuilder, ImageManifestBuilder, MediaType},
    };

    #[test]
    fn file_blob_store_round_trip() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let store = FileBlobStore::open(dir.path().join("blobs"))?;
        let record = store.put_bytes(b"hello")?;

        assert_eq!(
            record.digest,
            "sha256:2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
        assert!(store.exists(&record.digest)?);
        assert_eq!(store.read_bytes(&record.digest)?, b"hello");
        Ok(())
    }

    #[test]
    fn sqlite_index_store_round_trip() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let store = SqliteIndexStore::open(dir.path().join(SQLITE_INDEX_FILE_NAME))?;
        assert_eq!(store.schema_version()?, 1);

        let manifest_digest = sha256_digest(b"manifest");
        let layer_digest = sha256_digest(b"layer");

        store.put_blob(&BlobRecord {
            digest: manifest_digest.clone(),
            size: b"manifest".len() as u64,
            media_type: Some("application/vnd.oci.image.manifest.v1+json".to_string()),
            storage_uri: "blobs/sha256/manifest".to_string(),
            kind: BLOB_KIND_MANIFEST.to_string(),
            last_verified_at: None,
        })?;
        store.put_blob(&BlobRecord {
            digest: layer_digest.clone(),
            size: b"layer".len() as u64,
            media_type: Some("application/octet-stream".to_string()),
            storage_uri: "blobs/sha256/layer".to_string(),
            kind: BLOB_KIND_LAYER.to_string(),
            last_verified_at: None,
        })?;

        let manifest = ManifestRecord {
            digest: manifest_digest.clone(),
            media_type: "application/vnd.oci.image.manifest.v1+json".to_string(),
            size: b"manifest".len() as u64,
            subject_digest: None,
            annotations_json: "{}".to_string(),
            created_at: now_rfc3339(),
        };
        let layers = [LayerRecord {
            manifest_digest: manifest_digest.clone(),
            position: 0,
            digest: layer_digest.clone(),
            media_type: "application/octet-stream".to_string(),
            size: b"layer".len() as u64,
            annotations_json: "{}".to_string(),
        }];
        store.put_manifest(&manifest, &layers)?;
        store.put_ref("example.com/ommx/experiment", "latest", &manifest_digest)?;

        assert_eq!(store.get_blob(&layer_digest)?.unwrap().kind, "layer");
        assert_eq!(
            store.get_manifest(&manifest_digest)?.unwrap().media_type,
            "application/vnd.oci.image.manifest.v1+json"
        );
        let stored_layers = store.get_layers(&manifest_digest)?;
        assert_eq!(stored_layers, layers);
        assert_eq!(
            store.resolve_ref("example.com/ommx/experiment", "latest")?,
            Some(manifest_digest)
        );
        let refs = store.list_refs(Some("example.com/ommx"))?;
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].reference, "latest");
        Ok(())
    }

    #[test]
    fn concurrent_keep_existing_ref_publish_keeps_one_digest() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let root = dir.path().join("registry-v3");
        let index_store = SqliteIndexStore::open_in_registry_root(&root)?;
        let blob_store = FileBlobStore::open_in_registry_root(&root)?;
        let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/demo:race")?;
        let first_digest = put_test_manifest(&index_store, &blob_store, b"first-manifest")?;
        let second_digest = put_test_manifest(&index_store, &blob_store, b"second-manifest")?;
        assert_ne!(first_digest, second_digest);

        let handles: Vec<_> = [first_digest.clone(), second_digest.clone()]
            .into_iter()
            .map(|manifest_digest| {
                let root = root.clone();
                let image_name = image_name.clone();
                std::thread::spawn(move || -> Result<RefUpdate> {
                    let index_store = SqliteIndexStore::open_in_registry_root(root)?;
                    index_store.put_image_ref_with_policy(
                        &image_name,
                        &manifest_digest,
                        RefConflictPolicy::KeepExisting,
                    )
                })
            })
            .collect();

        let updates: Vec<_> = handles
            .into_iter()
            .map(|handle| handle.join().expect("ref publisher thread panicked"))
            .collect::<Result<_>>()?;

        assert_eq!(
            updates
                .iter()
                .filter(|update| matches!(update, RefUpdate::Inserted))
                .count(),
            1
        );
        assert_eq!(
            updates
                .iter()
                .filter(|update| matches!(update, RefUpdate::Conflicted { .. }))
                .count(),
            1
        );
        let final_digest = index_store
            .resolve_image_name(&image_name)?
            .context("Ref was not published")?;
        assert!(final_digest == first_digest || final_digest == second_digest);
        Ok(())
    }

    #[test]
    fn imports_legacy_oci_dir_into_sqlite_registry() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let legacy_dir = dir.path().join("legacy");
        let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/demo:v1")?;
        let layer = build_test_legacy_oci_dir(legacy_dir.clone(), image_name.clone())?;

        let registry_root = dir.path().join("registry-v3");
        let index_store = SqliteIndexStore::open_in_registry_root(&registry_root)?;
        let blob_store = FileBlobStore::open_in_registry_root(&registry_root)?;

        let imported = import_legacy_oci_dir(&index_store, &blob_store, &legacy_dir)?;

        assert_eq!(imported.image_name, Some(image_name.clone()));
        assert_eq!(
            index_store.resolve_image_name(&image_name)?,
            Some(imported.manifest_digest.clone())
        );
        assert!(blob_store.exists(&imported.manifest_digest)?);
        assert!(blob_store.exists(&layer.digest().to_string())?);

        let manifest = index_store
            .get_manifest(&imported.manifest_digest)?
            .context("Imported manifest is missing")?;
        let manifest_blob = index_store
            .get_blob(&manifest.digest)?
            .context("Imported manifest blob is missing")?;
        assert_eq!(manifest_blob.kind, BLOB_KIND_MANIFEST);
        let layers = index_store.get_layers(&imported.manifest_digest)?;
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].digest, layer.digest().to_string());
        let layer_blob = index_store
            .get_blob(&layers[0].digest)?
            .context("Imported layer blob is missing")?;
        assert_eq!(layer_blob.kind, BLOB_KIND_LAYER);
        Ok(())
    }

    #[test]
    fn migrates_legacy_local_registry_explicitly() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let legacy_registry_root = dir.path().join("legacy-registry");
        let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/demo:v2")?;
        let legacy_dir = legacy_local_registry_path(&legacy_registry_root, &image_name);
        build_test_legacy_oci_dir(legacy_dir, image_name.clone())?;

        let registry_root = dir.path().join("registry-v3");
        let index_store = SqliteIndexStore::open_in_registry_root(&registry_root)?;
        let blob_store = FileBlobStore::open_in_registry_root(&registry_root)?;

        assert!(index_store.resolve_image_name(&image_name)?.is_none());
        let report =
            migrate_legacy_local_registry(&index_store, &blob_store, &legacy_registry_root)?;
        assert_eq!(
            report,
            LegacyMigrationReport {
                scanned_dirs: 1,
                imported_dirs: 1,
                verified_dirs: 0,
                conflicted_dirs: 0,
                replaced_refs: 0
            }
        );
        let imported_digest = index_store
            .resolve_image_name(&image_name)?
            .context("Legacy local registry ref was not migrated")?;
        assert_eq!(
            migrate_legacy_local_registry(&index_store, &blob_store, &legacy_registry_root)?,
            LegacyMigrationReport {
                scanned_dirs: 1,
                imported_dirs: 0,
                verified_dirs: 1,
                conflicted_dirs: 0,
                replaced_refs: 0
            }
        );
        assert!(blob_store.exists(&imported_digest)?);
        Ok(())
    }

    #[test]
    fn migrate_legacy_local_registry_keeps_existing_ref_on_conflict() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let legacy_registry_root = dir.path().join("legacy-registry");
        let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/demo:conflict")?;
        let legacy_dir = legacy_local_registry_path(&legacy_registry_root, &image_name);
        build_test_legacy_oci_dir(legacy_dir.clone(), image_name.clone())?;
        let legacy_manifest_digest = legacy_oci_dir_ref(&legacy_dir)?.manifest_digest;

        let registry_root = dir.path().join("registry-v3");
        let index_store = SqliteIndexStore::open_in_registry_root(&registry_root)?;
        let blob_store = FileBlobStore::open_in_registry_root(&registry_root)?;
        let existing_digest =
            put_test_manifest_ref(&index_store, &blob_store, &image_name, b"existing-manifest")?;
        assert_ne!(existing_digest, legacy_manifest_digest);

        let report =
            migrate_legacy_local_registry(&index_store, &blob_store, &legacy_registry_root)?;
        assert_eq!(
            report,
            LegacyMigrationReport {
                scanned_dirs: 1,
                imported_dirs: 0,
                verified_dirs: 0,
                conflicted_dirs: 1,
                replaced_refs: 0
            }
        );
        assert_eq!(
            index_store.resolve_image_name(&image_name)?,
            Some(existing_digest)
        );
        assert!(!blob_store.exists(&legacy_manifest_digest)?);
        Ok(())
    }

    #[test]
    fn migrate_legacy_local_registry_replaces_existing_ref_when_requested() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let legacy_registry_root = dir.path().join("legacy-registry");
        let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/demo:replace")?;
        let legacy_dir = legacy_local_registry_path(&legacy_registry_root, &image_name);
        build_test_legacy_oci_dir(legacy_dir.clone(), image_name.clone())?;
        let legacy_manifest_digest = legacy_oci_dir_ref(&legacy_dir)?.manifest_digest;

        let registry_root = dir.path().join("registry-v3");
        let index_store = SqliteIndexStore::open_in_registry_root(&registry_root)?;
        let blob_store = FileBlobStore::open_in_registry_root(&registry_root)?;
        let existing_digest =
            put_test_manifest_ref(&index_store, &blob_store, &image_name, b"existing-manifest")?;
        assert_ne!(existing_digest, legacy_manifest_digest);

        let report = migrate_legacy_local_registry_with_policy(
            &index_store,
            &blob_store,
            &legacy_registry_root,
            RefConflictPolicy::Replace,
        )?;
        assert_eq!(
            report,
            LegacyMigrationReport {
                scanned_dirs: 1,
                imported_dirs: 0,
                verified_dirs: 0,
                conflicted_dirs: 0,
                replaced_refs: 1
            }
        );
        assert_eq!(
            index_store.resolve_image_name(&image_name)?,
            Some(legacy_manifest_digest.clone())
        );
        assert!(blob_store.exists(&legacy_manifest_digest)?);
        Ok(())
    }

    #[test]
    fn local_registry_migrates_legacy_refs_when_requested() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/demo:v3")?;
        let legacy_dir = legacy_local_registry_path(dir.path(), &image_name);
        build_test_legacy_oci_dir(legacy_dir, image_name.clone())?;

        let registry = LocalRegistry::open(dir.path())?;
        assert!(registry.resolve_image_name(&image_name)?.is_none());
        assert_eq!(
            registry.migrate_legacy_layout()?,
            LegacyMigrationReport {
                scanned_dirs: 1,
                imported_dirs: 1,
                verified_dirs: 0,
                conflicted_dirs: 0,
                replaced_refs: 0
            }
        );
        let imported_digest = registry
            .resolve_image_name(&image_name)?
            .context("Legacy local registry ref was not migrated")?;
        assert!(registry.blobs().exists(&imported_digest)?);
        assert!(registry.index().get_manifest(&imported_digest)?.is_some());
        Ok(())
    }

    #[test]
    fn concurrent_legacy_migrations_are_idempotent() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let root = dir.path().to_path_buf();
        let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/demo:parallel")?;
        let legacy_dir = legacy_local_registry_path(&root, &image_name);
        build_test_legacy_oci_dir(legacy_dir, image_name.clone())?;

        let handles: Vec<_> = (0..2)
            .map(|_| {
                let root = root.clone();
                std::thread::spawn(move || -> Result<LegacyMigrationReport> {
                    let registry = LocalRegistry::open(root)?;
                    registry.migrate_legacy_layout()
                })
            })
            .collect();

        let reports: Vec<_> = handles
            .into_iter()
            .map(|handle| handle.join().expect("migration thread panicked"))
            .collect::<Result<_>>()?;

        assert_eq!(
            reports
                .iter()
                .map(|report| report.scanned_dirs)
                .sum::<usize>(),
            2
        );
        assert_eq!(
            reports
                .iter()
                .map(|report| report.imported_dirs)
                .sum::<usize>(),
            1
        );
        assert_eq!(
            reports
                .iter()
                .map(|report| report.verified_dirs)
                .sum::<usize>(),
            1
        );
        assert_eq!(
            reports
                .iter()
                .map(|report| report.conflicted_dirs)
                .sum::<usize>(),
            0
        );

        let registry = LocalRegistry::open(&root)?;
        let imported_digest = registry
            .resolve_image_name(&image_name)?
            .context("Legacy local registry ref was not migrated")?;
        assert!(registry.blobs().exists(&imported_digest)?);
        Ok(())
    }

    #[test]
    fn concurrent_blob_writes_publish_one_complete_blob() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let root = dir.path().join("blobs");
        let bytes = b"parallel blob".to_vec();

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let root = root.clone();
                let bytes = bytes.clone();
                std::thread::spawn(move || -> Result<BlobRecord> {
                    let store = FileBlobStore::open(root)?;
                    store.put_bytes(&bytes)
                })
            })
            .collect();

        let records: Vec<_> = handles
            .into_iter()
            .map(|handle| handle.join().expect("blob writer thread panicked"))
            .collect::<Result<_>>()?;

        let digest = sha256_digest(&bytes);
        assert!(records.iter().all(|record| record.digest == digest));
        let store = FileBlobStore::open(&root)?;
        assert_eq!(store.read_bytes(&digest)?, bytes);
        Ok(())
    }

    fn put_test_manifest_ref(
        index_store: &SqliteIndexStore,
        blob_store: &FileBlobStore,
        image_name: &ImageName,
        bytes: &[u8],
    ) -> Result<String> {
        let digest = put_test_manifest(index_store, blob_store, bytes)?;
        index_store.put_image_ref(image_name, &digest)?;
        Ok(digest)
    }

    fn put_test_manifest(
        index_store: &SqliteIndexStore,
        blob_store: &FileBlobStore,
        bytes: &[u8],
    ) -> Result<String> {
        let mut blob = blob_store.put_bytes(bytes)?;
        blob.media_type = Some("application/vnd.oci.image.manifest.v1+json".to_string());
        blob.kind = BLOB_KIND_MANIFEST.to_string();
        index_store.put_blob(&blob)?;
        index_store.put_manifest(
            &ManifestRecord {
                digest: blob.digest.clone(),
                media_type: "application/vnd.oci.image.manifest.v1+json".to_string(),
                size: bytes.len() as u64,
                subject_digest: None,
                annotations_json: "{}".to_string(),
                created_at: now_rfc3339(),
            },
            &[],
        )?;
        Ok(blob.digest)
    }

    fn build_test_legacy_oci_dir(legacy_dir: PathBuf, image_name: ImageName) -> Result<Descriptor> {
        let mut builder = OciDirBuilder::new(legacy_dir, image_name)?;

        let config = builder.add_empty_json()?;
        let (layer_digest, layer_size) = builder.add_blob(b"instance")?;
        let layer = DescriptorBuilder::default()
            .media_type(MediaType::Other(
                "application/org.ommx.v1.instance".to_string(),
            ))
            .digest(layer_digest)
            .size(layer_size)
            .build()?;
        let manifest = ImageManifestBuilder::default()
            .schema_version(2_u32)
            .artifact_type(media_types::v1_artifact())
            .config(config)
            .layers(vec![layer.clone()])
            .build()?;
        let _oci_dir = builder.build(manifest)?;
        Ok(layer)
    }
}
