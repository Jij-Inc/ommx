use super::{
    now_rfc3339, sha256_digest, FileBlobStore, LayerRecord, ManifestRecord, RefConflictPolicy,
    RefUpdate, SqliteIndexStore, ValidatedDigest, BLOB_KIND_CONFIG, BLOB_KIND_LAYER,
    BLOB_KIND_MANIFEST, OCI_IMAGE_REF_NAME_ANNOTATION,
};
use anyhow::{ensure, Context, Result};
use ocipkg::{
    oci_spec::image::{Descriptor, ImageIndex, ImageManifest, OciLayout},
    ImageName,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

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
                    policy,
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
                    policy,
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
    let digest = ValidatedDigest::parse(digest)?;
    Ok(oci_dir_root
        .join("blobs")
        .join(digest.algorithm())
        .join(digest.encoded()))
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
