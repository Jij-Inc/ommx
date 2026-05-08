//! v2 OMMX local registry compatibility.
//!
//! The OMMX v2 local registry stores each `(image_name, tag)` as a
//! standalone OCI Image Layout directory under
//! `<root>/<image_name_path>/<tag>/`. v3 replaces this layout with the
//! SQLite-backed `LocalRegistry` (see [`super::registry`]), but
//! existing v2 caches must remain readable via an explicit one-shot
//! import.
//!
//! This module owns only the **v2-shape-specific** helpers:
//!
//! - the path computation `<root>/<image_name>` ([`legacy_local_registry_path`]),
//! - the recursive scan of a v2 root for `oci-layout`-bearing dirs
//!   ([`gather_legacy_oci_dirs`]),
//! - per-entry name resolution that reconciles the on-disk path with the
//!   manifest's `org.opencontainers.image.ref.name` annotation
//!   ([`legacy_import_image_name`]),
//! - the batch entry points that turn a v2 root into a series of
//!   identity-preserving imports ([`import_legacy_local_registry`] and
//!   the `_with_policy` / `_ref` variants), and the aggregated
//!   [`LegacyImportReport`].
//!
//! Reading and importing one OCI Image Layout directory in isolation is
//! **not** v2-specific and lives in [`super::oci_dir`]; this module just
//! drives that lower layer with v2-aware bookkeeping.

use super::super::{FileBlobStore, RefConflictPolicy, RefUpdate, SqliteIndexStore};
use super::oci_dir::{
    import_oci_dir_as_ref_with_policy, import_oci_dir_as_ref_with_policy_inner, oci_dir_image_name,
    oci_dir_ref, OciDirImport, RefConflictHandling,
};
use anyhow::{ensure, Context, Result};
use ocipkg::ImageName;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Aggregate outcome of an [`import_legacy_local_registry`] run.
///
/// `#[non_exhaustive]` so future counters (e.g. orphan-blob discovery
/// during the v2 sweep, byte counts) can be added without breaking
/// exhaustive struct literal construction at call sites.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyImportReport {
    pub scanned_dirs: usize,
    pub imported_dirs: usize,
    pub verified_dirs: usize,
    pub conflicted_dirs: usize,
    pub replaced_refs: usize,
}

impl LegacyImportReport {
    fn empty(scanned_dirs: usize) -> Self {
        Self {
            scanned_dirs,
            imported_dirs: 0,
            verified_dirs: 0,
            conflicted_dirs: 0,
            replaced_refs: 0,
        }
    }
}

pub fn import_legacy_local_registry_ref(
    index_store: &SqliteIndexStore,
    blob_store: &FileBlobStore,
    legacy_registry_root: impl AsRef<Path>,
    image_name: &ImageName,
) -> Result<OciDirImport> {
    import_legacy_local_registry_ref_with_policy(
        index_store,
        blob_store,
        legacy_registry_root,
        image_name,
        RefConflictPolicy::KeepExisting,
    )
}

pub fn import_legacy_local_registry_ref_with_policy(
    index_store: &SqliteIndexStore,
    blob_store: &FileBlobStore,
    legacy_registry_root: impl AsRef<Path>,
    image_name: &ImageName,
    policy: RefConflictPolicy,
) -> Result<OciDirImport> {
    let legacy_path = legacy_local_registry_path(legacy_registry_root, image_name);
    import_oci_dir_as_ref_with_policy(index_store, blob_store, legacy_path, image_name, policy)
}

pub fn import_legacy_local_registry(
    index_store: &SqliteIndexStore,
    blob_store: &FileBlobStore,
    legacy_registry_root: impl AsRef<Path>,
) -> Result<LegacyImportReport> {
    import_legacy_local_registry_with_policy(
        index_store,
        blob_store,
        legacy_registry_root,
        RefConflictPolicy::KeepExisting,
    )
}

pub fn import_legacy_local_registry_with_policy(
    index_store: &SqliteIndexStore,
    blob_store: &FileBlobStore,
    legacy_registry_root: impl AsRef<Path>,
    policy: RefConflictPolicy,
) -> Result<LegacyImportReport> {
    let legacy_registry_root = legacy_registry_root.as_ref();
    let legacy_dirs = gather_legacy_oci_dirs(legacy_registry_root)?;
    let mut report = LegacyImportReport::empty(legacy_dirs.len());

    for legacy_dir in &legacy_dirs {
        let image_name = legacy_import_image_name(legacy_registry_root, legacy_dir)?;
        let dir_ref = oci_dir_ref(legacy_dir)?;
        let existing_manifest_digest = index_store.resolve_image_name(&image_name)?;

        match existing_manifest_digest {
            None => {
                let (_, ref_update) = import_oci_dir_as_ref_with_policy_inner(
                    index_store,
                    blob_store,
                    legacy_dir,
                    &image_name,
                    policy,
                    RefConflictHandling::Return,
                )
                .with_context(|| {
                    format!(
                        "Failed to import legacy local registry entry {}",
                        legacy_dir.display()
                    )
                })?;
                record_import_ref_update(&mut report, ref_update);
            }
            Some(existing) if existing == dir_ref.manifest_digest => {
                let (_, ref_update) = import_oci_dir_as_ref_with_policy_inner(
                    index_store,
                    blob_store,
                    legacy_dir,
                    &image_name,
                    policy,
                    RefConflictHandling::Return,
                )
                .with_context(|| {
                    format!(
                        "Failed to verify imported legacy local registry entry {}",
                        legacy_dir.display()
                    )
                })?;
                record_import_ref_update(&mut report, ref_update);
            }
            Some(_) if policy == RefConflictPolicy::KeepExisting => {
                report.conflicted_dirs += 1;
            }
            Some(_) => {
                let (_, ref_update) = import_oci_dir_as_ref_with_policy_inner(
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
                record_import_ref_update(&mut report, ref_update);
            }
        }
    }

    Ok(report)
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

fn legacy_import_image_name(legacy_registry_root: &Path, legacy_dir: &Path) -> Result<ImageName> {
    let annotated = oci_dir_image_name(legacy_dir)?;
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

fn record_import_ref_update(report: &mut LegacyImportReport, update: RefUpdate) {
    match update {
        RefUpdate::Inserted => report.imported_dirs += 1,
        RefUpdate::Unchanged => report.verified_dirs += 1,
        RefUpdate::Replaced { .. } => report.replaced_refs += 1,
        RefUpdate::Conflicted { .. } => report.conflicted_dirs += 1,
    }
}
