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
//!   backed by the v2 disk-cache encoding [`image_ref_as_path`] /
//!   [`image_ref_from_path`] (`__` substituted for `:`),
//! - the recursive scan of a v2 root for `oci-layout`-bearing dirs
//!   ([`gather_legacy_oci_dirs`]),
//! - per-entry name resolution that reconciles the on-disk path with the
//!   manifest's `org.opencontainers.image.ref.name` annotation
//!   ([`legacy_import_image_name`]),
//! - the batch entry points that turn a v2 root into a series of
//!   identity-preserving imports ([`import_legacy_local_registry`] and
//!   the `replace_*` / `_ref` variants), and the aggregated
//!   [`LegacyImportReport`].
//!
//! Reading and importing one OCI Image Layout directory in isolation is
//! **not** v2-specific and lives in [`super::oci_dir`]; this module just
//! drives that lower layer with v2-aware bookkeeping.

use super::super::{FileBlobStore, RefUpdate, SqliteIndexStore};
use super::oci_dir::{
    import_oci_dir_as_ref, import_oci_dir_inner, oci_dir_image_name, oci_dir_ref,
    replace_oci_dir_as_ref, OciDirImport, RefConflictHandling, RefWriteMode,
};
use crate::artifact::ImageRef;
use anyhow::{ensure, Context, Result};
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
    image_name: &ImageRef,
) -> Result<OciDirImport> {
    let legacy_path = legacy_local_registry_path(legacy_registry_root, image_name);
    import_oci_dir_as_ref(index_store, blob_store, legacy_path, image_name)
}

pub fn replace_legacy_local_registry_ref(
    index_store: &SqliteIndexStore,
    blob_store: &FileBlobStore,
    legacy_registry_root: impl AsRef<Path>,
    image_name: &ImageRef,
) -> Result<OciDirImport> {
    let legacy_path = legacy_local_registry_path(legacy_registry_root, image_name);
    replace_oci_dir_as_ref(index_store, blob_store, legacy_path, image_name)
}

pub fn import_legacy_local_registry(
    index_store: &SqliteIndexStore,
    blob_store: &FileBlobStore,
    legacy_registry_root: impl AsRef<Path>,
) -> Result<LegacyImportReport> {
    import_legacy_local_registry_inner(
        index_store,
        blob_store,
        legacy_registry_root,
        RefWriteMode::Publish,
    )
}

pub fn replace_legacy_local_registry(
    index_store: &SqliteIndexStore,
    blob_store: &FileBlobStore,
    legacy_registry_root: impl AsRef<Path>,
) -> Result<LegacyImportReport> {
    import_legacy_local_registry_inner(
        index_store,
        blob_store,
        legacy_registry_root,
        RefWriteMode::Replace,
    )
}

fn import_legacy_local_registry_inner(
    index_store: &SqliteIndexStore,
    blob_store: &FileBlobStore,
    legacy_registry_root: impl AsRef<Path>,
    write_mode: RefWriteMode,
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
                let (_, ref_update) = import_oci_dir_inner(
                    index_store,
                    blob_store,
                    legacy_dir,
                    Some(&image_name),
                    write_mode,
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
                let (_, ref_update) = import_oci_dir_inner(
                    index_store,
                    blob_store,
                    legacy_dir,
                    Some(&image_name),
                    write_mode,
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
            Some(_) if write_mode == RefWriteMode::Publish => {
                report.conflicted_dirs += 1;
            }
            Some(_) => {
                let (_, ref_update) = import_oci_dir_inner(
                    index_store,
                    blob_store,
                    legacy_dir,
                    Some(&image_name),
                    RefWriteMode::Replace,
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
    image_name: &ImageRef,
) -> PathBuf {
    legacy_registry_root
        .as_ref()
        .join(image_ref_as_path(image_name))
}

/// Encode an [`ImageRef`] as the v2 disk-cache path
/// `{hostname}/{name}/__{reference}` (or
/// `{hostname}__{port}/{name}/__{reference}` when a port is set).
/// This is the layout SDK v2 wrote to disk per `(image, tag)`; v3
/// no longer produces this layout but still needs to read it during
/// `ommx artifact import`.
///
/// The encoding maps `:` to `__`, inherited byte-for-byte from
/// SDK v2. Tags that legitimately contain `__` (which the OCI
/// distribution tag grammar otherwise allows) are ambiguous on
/// round-trip — [`image_ref_from_path`] decodes `__` back to `:`,
/// so a tag `my__tag` becomes the digest-shaped `my:tag` and fails
/// `oci_spec::distribution::Reference`'s digest-length check.
/// OMMX-generated refs never use `__` in tags, so the v2 → v3
/// import path is unaffected. Switching to a percent-encoded layout
/// would invalidate existing v2 caches on disk, so the legacy
/// encoding is preserved.
pub(crate) fn image_ref_as_path(image_name: &ImageRef) -> PathBuf {
    let reference = image_name.reference().replace(':', "__");
    // v2 disk layout encodes `host:port` as `host__port`. Split out
    // the port from the canonical `host[:port]` form at the call
    // site rather than via dedicated accessors — the split is a
    // local detail of the legacy encoding, not a v3 concept on
    // [`ImageRef`].
    let host = match image_name.registry().rsplit_once(':') {
        Some((host, port)) => format!("{host}__{port}"),
        None => image_name.registry().to_string(),
    };
    PathBuf::from(format!("{host}/{}/__{reference}", image_name.name()))
}

/// Inverse of [`image_ref_as_path`]. Returns an error when the
/// path shape doesn't match the encoding, so a stray directory
/// inside the legacy local registry root surfaces a clear error
/// during import rather than producing a corrupted ref.
pub(crate) fn image_ref_from_path(path: &Path) -> Result<ImageRef> {
    let components = path
        .components()
        .map(|c| {
            c.as_os_str()
                .to_str()
                .context("Path includes a non UTF-8 component")
        })
        .collect::<Result<Vec<&str>>>()?;
    if components.len() < 3 {
        anyhow::bail!(
            "Path for image ref must contain registry, name, and tag components: {}",
            path.display()
        );
    }
    let registry = components[0].replace("__", ":");
    let n = components.len();
    let name = components[1..n - 1].join("/");
    let last = components[n - 1]
        .strip_prefix("__")
        .with_context(|| format!("Missing tag prefix in path: {}", path.display()))?
        .replace("__", ":");
    ImageRef::from_repository_and_reference(&format!("{registry}/{name}"), &last)
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

fn legacy_import_image_name(legacy_registry_root: &Path, legacy_dir: &Path) -> Result<ImageRef> {
    let annotated = oci_dir_image_name(legacy_dir)?;
    let path_name = legacy_dir
        .strip_prefix(legacy_registry_root)
        .ok()
        .and_then(|relative| image_ref_from_path(relative).ok());

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

fn record_import_ref_update(report: &mut LegacyImportReport, update: Option<RefUpdate>) {
    // The legacy batch import always passes an explicit `image_name`
    // to `import_oci_dir_inner`, so the inner function always writes
    // a ref and the outcome is always `Some(_)`.
    let Some(update) = update else { return };
    match update {
        RefUpdate::Inserted => report.imported_dirs += 1,
        RefUpdate::Unchanged => report.verified_dirs += 1,
        RefUpdate::Replaced { .. } => report.replaced_refs += 1,
        RefUpdate::Conflicted { .. } => report.conflicted_dirs += 1,
    }
}

#[cfg(test)]
mod path_layout_tests {
    use super::*;

    /// Path round-trip holds for every ref whose tag does not contain
    /// `__` — the v2-inherited encoding maps `:` to `__`, so a tag
    /// already containing `__` is the documented break point.
    #[test]
    fn round_trip_path_layout_for_non_underscore_tags() {
        for input in [
            "localhost:5000/test_repo:latest",
            "ubuntu:20.04",
            "alpine",
            "quay.io/jitesoft/alpine@sha256:6755355f801f8e3694bffb1a925786813462cea16f1ce2b0290b6a48acf2500c",
        ] {
            let r = ImageRef::parse(input).unwrap();
            let path = image_ref_as_path(&r);
            let parsed = image_ref_from_path(&path).unwrap();
            assert_eq!(parsed, r, "round-trip failed for {input}");
        }
    }

    /// Tags that legitimately contain `__` collide with the legacy
    /// path encoding of `:`, so the round-trip is not lossless —
    /// `image_ref_from_path` decodes every `__` back to `:`, then
    /// reassembles the ref with `@` as the digest separator. The
    /// result fails `oci_spec::distribution::Reference`'s digest
    /// length check (a "tag" like `my__tag` won't satisfy any known
    /// `algorithm:hex` shape), surfacing the lossy case as a clear
    /// parse error rather than silent corruption.
    #[test]
    fn path_layout_round_trip_fails_for_double_underscore_tags() {
        let r = ImageRef::parse("example.com/foo:my__tag").unwrap();
        let path = image_ref_as_path(&r);
        assert!(
            image_ref_from_path(&path).is_err(),
            "image_ref_from_path should reject the lossy round-trip rather than corrupt the ref",
        );
    }
}
