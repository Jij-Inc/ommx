//! Standard OCI Image Layout directory I/O for the v3 Local Registry.
//!
//! This module is **not legacy**. It deals with the OCI standard
//! `oci-layout` + `index.json` + `blobs/<algorithm>/<encoded>` directory
//! format, regardless of where the directory came from:
//!
//! - a v2 OMMX local registry path/tag entry,
//! - an explicit export from `oras` / `crane` / `skopeo`,
//! - a `.ommx` archive that has been expanded to a directory by hand,
//! - or a fresh export produced by the v3 SDK itself in the future.
//!
//! v3 import is **identity-preserving**: the manifest bytes and digest
//! that come out of the directory are stored verbatim into the
//! Local Registry index and CAS. Reformatting an OCI
//! Image Manifest into an OCI Artifact Manifest is intentionally a
//! separate explicit `convert` operation that produces a new artifact
//! under a new digest / new ref, not a side effect of import.
//!
//! The v2-OMMX-local-registry-specific code (the recursive scan of
//! a path/tag-shaped tree, the path-to-`ImageRef` heuristics) lives
//! in the sibling [`super::legacy`] module and uses this module's
//! primitives.

use super::super::{
    sha256_digest, LocalRegistry, RefUpdate, ValidatedDigest, OCI_IMAGE_REF_NAME_ANNOTATION,
};
use crate::artifact::{media_types, ImageRef};
use anyhow::{ensure, Context, Result};
use oci_spec::image::{Descriptor, Digest, ImageIndex, ImageManifest, MediaType, OciLayout};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Identity of an OCI Image Layout directory entry: the manifest
/// digest preserved from `index.json`, plus the OCI image ref name
/// annotation when present.
///
/// Returned by [`OciDirRef::read`] (pure lookup with no v3 registry side
/// effects). Import paths return [`OciDirImport`] instead, which carries the
/// published ref plus the [`RefUpdate`] describing what happened to the SQLite
/// ref.
///
/// Marked `#[non_exhaustive]` so future identity attributes (for
/// example a typed media type) can be added without breaking
/// exhaustive destructuring at call sites.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OciDirRef {
    pub manifest_digest: Digest,
    pub image_name: Option<ImageRef>,
}

/// Outcome of importing a single OCI Image Layout directory into the
/// v3 SQLite Local Registry.
///
/// `manifest_digest` and `image_name` carry the same identity that
/// the registry published. If the source directory does not carry an
/// OCI ref-name annotation, [`LocalRegistry::import_oci_dir`] synthesizes
/// an anonymous local image name so the imported artifact is still
/// addressable by ref.
///
/// `ref_update` distinguishes the four outcomes the SQLite ref
/// transition can take:
///
/// - `Inserted` — fresh ref published.
/// - `Unchanged` — same `image_name` already pointed at the same
///   manifest digest; the import was an idempotent verify.
/// - `Replaced { previous_manifest_digest }` — a replace entry
///   point overwrote an older digest.
/// - `Conflicted { existing, incoming }` — only seen by callers
///   that drive the import with `RefConflictHandling::Return` (e.g.
///   the legacy batch importer); public import methods surface a
///   conflict as `Result::Err`.
///
/// `#[non_exhaustive]` so future fields (for example, the number of
/// blobs newly written versus reused via CAS dedup) are additive.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OciDirImport {
    pub manifest_digest: Digest,
    pub image_name: ImageRef,
    pub ref_update: RefUpdate,
}

impl OciDirImport {
    fn from_inner(ref_info: ImportedOciDirRef, ref_update: RefUpdate) -> Self {
        Self {
            manifest_digest: ref_info.manifest_digest,
            image_name: ref_info.image_name,
            ref_update,
        }
    }
}

/// Whether ref conflicts should bail out or be returned to the caller.
///
/// `Error` is the public API default — most callers want a conflict to
/// surface as a `Result::Err`. `Return` is used by the v2 batch import
/// in [`super::legacy`] which aggregates per-directory outcomes into a
/// single `LegacyImportReport`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RefConflictHandling {
    Error,
    Return,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RefWriteMode {
    Publish,
    Replace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RefSelection<'a> {
    SourceOrAnonymous,
    Explicit(&'a ImageRef),
}

/// The single manifest entry read from an OCI Image Layout directory.
///
/// This is source-side data, not Local Registry state: it carries the
/// source identity (digest + ref-name annotation), the manifest bytes /
/// descriptor that get imported verbatim, the layer descriptors
/// enumerated from the manifest, and the config blob bytes.
///
/// Built once by [`read_oci_dir_entry`] and consumed by
/// [`import_oci_dir_inner`]. v3 import is identity-preserving:
/// `manifest_bytes` and `manifest_digest` are stored verbatim. The only
/// supported manifest format is OCI Image Manifest (with `artifactType`
/// set to the OMMX artifact media type); the deprecated OCI Artifact
/// Manifest is rejected at parse time.
struct OciDirEntry {
    manifest_digest: Digest,
    image_name: Option<ImageRef>,
    manifest_bytes: Vec<u8>,
    manifest_descriptor: Descriptor,
    layers: Vec<Descriptor>,
    image_config: (Descriptor, Vec<u8>),
}

pub(super) struct ImportedOciDirRef {
    manifest_digest: Digest,
    image_name: ImageRef,
}

impl LocalRegistry {
    /// Import a standard OCI Image Layout directory into this Local Registry.
    ///
    /// Works for any OCI Image Layout (`oci-layout` + `index.json` + `blobs/`)
    /// that uses OCI Image Manifest with the OMMX `artifactType` set. The
    /// registry reads the source layout only to discover and verify the single
    /// manifest, then stores the exact content-addressed blobs in the Local
    /// Registry. The manifest digest is preserved verbatim: import never
    /// rewrites the manifest.
    ///
    /// If the source directory has no `org.opencontainers.image.ref.name`
    /// annotation, the registry synthesizes an anonymous local ref so the
    /// imported artifact is addressable.
    pub fn import_oci_dir(&self, oci_dir_root: impl AsRef<Path>) -> Result<OciDirImport> {
        let (ref_info, ref_update) = self.import_oci_dir_inner(
            oci_dir_root,
            RefSelection::SourceOrAnonymous,
            RefWriteMode::Publish,
            RefConflictHandling::Error,
        )?;
        Ok(OciDirImport::from_inner(ref_info, ref_update))
    }

    pub fn replace_oci_dir(&self, oci_dir_root: impl AsRef<Path>) -> Result<OciDirImport> {
        let (ref_info, ref_update) = self.import_oci_dir_inner(
            oci_dir_root,
            RefSelection::SourceOrAnonymous,
            RefWriteMode::Replace,
            RefConflictHandling::Error,
        )?;
        Ok(OciDirImport::from_inner(ref_info, ref_update))
    }

    pub(in crate::artifact::local_registry) fn import_oci_dir_as_ref(
        &self,
        oci_dir_root: impl AsRef<Path>,
        image_name: &ImageRef,
    ) -> Result<OciDirImport> {
        let (ref_info, ref_update) = self.import_oci_dir_inner(
            oci_dir_root,
            RefSelection::Explicit(image_name),
            RefWriteMode::Publish,
            RefConflictHandling::Error,
        )?;
        Ok(OciDirImport::from_inner(ref_info, ref_update))
    }

    pub(super) fn replace_oci_dir_as_ref_inner(
        &self,
        oci_dir_root: impl AsRef<Path>,
        image_name: &ImageRef,
    ) -> Result<OciDirImport> {
        let (ref_info, ref_update) = self.import_oci_dir_inner(
            oci_dir_root,
            RefSelection::Explicit(image_name),
            RefWriteMode::Replace,
            RefConflictHandling::Error,
        )?;
        Ok(OciDirImport::from_inner(ref_info, ref_update))
    }

    /// Unified implementation used by public OCI-dir import and by the v2
    /// batch import in [`super::legacy`].
    pub(super) fn import_oci_dir_inner(
        &self,
        oci_dir_root: impl AsRef<Path>,
        ref_selection: RefSelection<'_>,
        write_mode: RefWriteMode,
        conflict_handling: RefConflictHandling,
    ) -> Result<(ImportedOciDirRef, RefUpdate)> {
        let oci_dir_root = oci_dir_root.as_ref();
        let entry = read_oci_dir_entry(oci_dir_root)?;
        let manifest_digest = entry.manifest_digest.clone();
        let effective_image_name = self.resolve_import_ref(oci_dir_root, &entry, ref_selection)?;

        // Pre-check: under publish semantics, surface the conflict before we
        // store any CAS bytes. The atomic publish re-validates
        // the same condition inside the SQLite transaction, so concurrent
        // writers still get a consistent outcome; this is purely a fast
        // path for the common single-writer case.
        if write_mode == RefWriteMode::Publish {
            if let Some(existing_descriptor) = self
                .index()
                .resolve_image_descriptor(&effective_image_name)?
            {
                if existing_descriptor.digest() != entry.manifest_descriptor.digest() {
                    if conflict_handling == RefConflictHandling::Error {
                        anyhow::bail!(
                            "Local registry ref conflict for {}: existing manifest {}, incoming manifest {}",
                            effective_image_name,
                            existing_descriptor.digest(),
                            entry.manifest_descriptor.digest(),
                        );
                    }
                    let conflict = RefUpdate::Conflicted {
                        existing_manifest_digest: existing_descriptor.digest().clone(),
                        incoming_manifest_digest: entry.manifest_descriptor.digest().clone(),
                    };
                    return Ok((
                        ImportedOciDirRef {
                            manifest_digest,
                            image_name: effective_image_name,
                        },
                        conflict,
                    ));
                }
            }
        }

        // Store source bytes for layers, config, and the manifest itself.
        // These writes are idempotent and independent of the SQLite ref
        // index.
        for layer in entry.layers.as_slice() {
            self.store_oci_dir_descriptor_blob(oci_dir_root, layer)?;
        }
        let (config_descriptor, config_bytes) = &entry.image_config;
        self.store_descriptor_bytes(config_descriptor, config_bytes)?;
        self.store_descriptor_bytes(&entry.manifest_descriptor, &entry.manifest_bytes)?;

        let ref_update = match write_mode {
            RefWriteMode::Publish => self
                .index()
                .publish_image_ref(&effective_image_name, &entry.manifest_descriptor)?,
            RefWriteMode::Replace => self
                .index()
                .replace_image_ref(&effective_image_name, &entry.manifest_descriptor)?,
        };
        if let RefUpdate::Conflicted {
            existing_manifest_digest,
            incoming_manifest_digest,
        } = &ref_update
        {
            if conflict_handling == RefConflictHandling::Error {
                anyhow::bail!(
                    "Local registry ref conflict for {}: existing manifest {}, incoming manifest {}",
                    effective_image_name,
                    existing_manifest_digest,
                    incoming_manifest_digest
                );
            }
        }

        Ok((
            ImportedOciDirRef {
                manifest_digest,
                image_name: effective_image_name,
            },
            ref_update,
        ))
    }

    fn store_oci_dir_descriptor_blob(&self, oci_dir_root: &Path, desc: &Descriptor) -> Result<()> {
        let digest = digest_to_string(desc.digest());
        let bytes = read_oci_dir_blob(oci_dir_root, &digest)
            .with_context(|| format!("Failed to read blob {digest}"))?;
        ensure!(
            bytes.len() as u64 == desc.size(),
            "Blob size mismatch for {digest}: descriptor={}, actual={}",
            desc.size(),
            bytes.len()
        );
        self.store_descriptor_bytes(desc, &bytes)
    }

    fn store_descriptor_bytes(&self, desc: &Descriptor, bytes: &[u8]) -> Result<()> {
        self.store_blob(desc.clone(), bytes)?;
        Ok(())
    }

    fn resolve_import_ref(
        &self,
        oci_dir_root: &Path,
        entry: &OciDirEntry,
        ref_selection: RefSelection<'_>,
    ) -> Result<ImageRef> {
        match ref_selection {
            RefSelection::SourceOrAnonymous => {
                if let Some(image_name) = entry.image_name.clone() {
                    Ok(image_name)
                } else {
                    let synthesized = self.synthesize_anonymous_image_name()?;
                    tracing::info!(
                        "OCI dir at {} has no `org.opencontainers.image.ref.name` \
                         annotation; importing under synthesized anonymous name {synthesized}",
                        oci_dir_root.display(),
                    );
                    Ok(synthesized)
                }
            }
            RefSelection::Explicit(target) => {
                if let Some(annotated) = entry.image_name.as_ref() {
                    ensure!(
                        target == annotated,
                        "OCI dir ref mismatch: requested={}, annotated={}",
                        target,
                        annotated
                    );
                }
                Ok(target.clone())
            }
        }
    }
}

impl OciDirRef {
    pub fn read(oci_dir_root: impl AsRef<Path>) -> Result<Self> {
        let entry = read_oci_dir_entry(oci_dir_root)?;
        Ok(Self {
            manifest_digest: entry.manifest_digest,
            image_name: entry.image_name,
        })
    }

    pub fn image_name(oci_dir_root: impl AsRef<Path>) -> Result<Option<ImageRef>> {
        Ok(Self::read(oci_dir_root)?.image_name)
    }
}

fn ensure_oci_layout(oci_dir_root: &Path) -> Result<()> {
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

fn read_oci_dir_entry(oci_dir_root: impl AsRef<Path>) -> Result<OciDirEntry> {
    let oci_dir_root = oci_dir_root.as_ref();
    ensure_oci_layout(oci_dir_root)?;

    let index_path = oci_dir_root.join("index.json");
    let image_index: ImageIndex = read_json_file(&index_path)?;
    ensure!(
        image_index.manifests().len() == 1,
        "OMMX OCI dir entry must contain exactly one manifest: {}",
        index_path.display()
    );
    let index_descriptor = image_index.manifests().first().unwrap();
    let image_name = image_name_from_index_descriptor(index_descriptor)?;
    let manifest_digest = index_descriptor.digest().clone();
    let manifest_digest_str = digest_to_string(&manifest_digest);
    let manifest_bytes = read_oci_dir_blob(oci_dir_root, &manifest_digest_str)
        .with_context(|| format!("Failed to read manifest blob {manifest_digest}"))?;
    ensure!(
        manifest_bytes.len() as u64 == index_descriptor.size(),
        "Manifest blob size mismatch for {manifest_digest}: descriptor={}, actual={}",
        index_descriptor.size(),
        manifest_bytes.len()
    );

    // v3 supports only OCI Image Manifest. The deprecated OCI Artifact
    // Manifest is rejected at parse time.
    let (layers, image_config) = match index_descriptor.media_type() {
        MediaType::ImageManifest => read_image_manifest_fields(oci_dir_root, &manifest_bytes)?,
        MediaType::ArtifactManifest => anyhow::bail!(
            "OCI dir uses the deprecated OCI Artifact Manifest \
             (application/vnd.oci.artifact.manifest.v1+json), which is not supported. \
             v3 OMMX accepts only OCI Image Manifest with artifactType."
        ),
        other => anyhow::bail!(
            "OCI dir manifest descriptor has unsupported media type: {other}. \
             Expected an OMMX Image Manifest."
        ),
    };

    Ok(OciDirEntry {
        manifest_digest,
        image_name,
        manifest_bytes,
        manifest_descriptor: index_descriptor.clone(),
        layers,
        image_config,
    })
}

/// Manifest-derived fields filled into [`OciDirEntry`] by
/// [`read_oci_dir_entry`].
type ImageManifestFields = (Vec<Descriptor>, (Descriptor, Vec<u8>));

fn read_image_manifest_fields(
    oci_dir_root: &Path,
    manifest_bytes: &[u8],
) -> Result<ImageManifestFields> {
    let manifest: ImageManifest =
        serde_json::from_slice(manifest_bytes).context("Failed to parse OCI image manifest")?;
    ensure_ommx_artifact_type(manifest.artifact_type().as_ref())?;

    // Image Manifest references a config blob; read it so the registry
    // can re-export a self-contained OCI Image Layout later.
    let config_descriptor = manifest.config().clone();
    let config_digest = digest_to_string(config_descriptor.digest());
    let config_bytes = read_oci_dir_blob(oci_dir_root, &config_digest)
        .with_context(|| format!("Failed to read config blob {config_digest}"))?;
    ensure!(
        config_bytes.len() as u64 == config_descriptor.size(),
        "Config blob size mismatch for {config_digest}: descriptor={}, actual={}",
        config_descriptor.size(),
        config_bytes.len()
    );

    Ok((
        manifest.layers().to_vec(),
        (config_descriptor, config_bytes),
    ))
}

fn ensure_ommx_artifact_type(artifact_type: Option<&MediaType>) -> Result<()> {
    let artifact_type =
        artifact_type.context("OCI dir is not an OMMX artifact: artifactType is missing")?;
    ensure!(
        artifact_type == &media_types::v1_artifact(),
        "OCI dir is not an OMMX artifact: {}",
        artifact_type
    );
    Ok(())
}

fn read_oci_dir_blob(oci_dir_root: &Path, digest: &str) -> Result<Vec<u8>> {
    let path = oci_dir_blob_path(oci_dir_root, digest)?;
    let bytes = fs::read(&path).with_context(|| format!("Failed to read {}", path.display()))?;
    ensure!(
        sha256_digest(&bytes) == digest,
        "OCI dir blob digest verification failed for {digest}"
    );
    Ok(bytes)
}

fn oci_dir_blob_path(oci_dir_root: &Path, digest: &str) -> Result<PathBuf> {
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

fn image_name_from_index_descriptor(desc: &Descriptor) -> Result<Option<ImageRef>> {
    desc.annotations()
        .as_ref()
        .and_then(|annotations| annotations.get(OCI_IMAGE_REF_NAME_ANNOTATION))
        .map(|name| ImageRef::parse(name).with_context(|| format!("Invalid image ref: {name}")))
        .transpose()
}

fn digest_to_string<D: std::fmt::Display + ?Sized>(digest: &D) -> String {
    digest.to_string()
}
