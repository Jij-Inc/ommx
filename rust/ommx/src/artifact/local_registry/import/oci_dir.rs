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
//! that come out of the directory are stored verbatim into
//! [`SqliteIndexStore`] and [`FileBlobStore`]. Reformatting an OCI
//! Image Manifest into an OCI Artifact Manifest is intentionally a
//! separate explicit `convert` operation that produces a new artifact
//! under a new digest / new ref, not a side effect of import.
//!
//! The v2-OMMX-local-registry-specific code (the recursive scan of
//! a path/tag-shaped tree, the path-to-`ImageName` heuristics) lives
//! in the sibling [`super::legacy`] module and uses this module's
//! primitives.

use super::super::{
    annotations_json, now_rfc3339, sha256_digest, BlobRecord, FileBlobStore, LayerRecord,
    ManifestRecord, RefConflictPolicy, RefUpdate, SqliteIndexStore, ValidatedDigest,
    BLOB_KIND_BLOB, BLOB_KIND_CONFIG, BLOB_KIND_MANIFEST, OCI_IMAGE_REF_NAME_ANNOTATION,
};
use crate::artifact::{media_types, OCI_IMAGE_MANIFEST_MEDIA_TYPE};
use anyhow::{ensure, Context, Result};
use oci_spec::image::{
    Descriptor, DescriptorBuilder, Digest, ImageIndex, ImageManifest, MediaType, OciLayout,
};
use ocipkg::ImageName;
use std::collections::HashMap;
use std::{
    fs,
    path::{Path, PathBuf},
    str::FromStr,
};

/// Identity of an OCI Image Layout directory entry: the manifest
/// digest preserved from `index.json`, plus the OCI image ref name
/// annotation when present.
///
/// Returned by [`oci_dir_ref`] (pure lookup with no v3 registry side
/// effects). Identity-preserving import paths return [`OciDirImport`]
/// instead, which carries the same fields plus the `RefUpdate`
/// describing what happened to the SQLite ref.
///
/// Marked `#[non_exhaustive]` so future identity attributes (for
/// example a parsed `oci_spec::image::Digest` or a typed media type)
/// can be added without breaking exhaustive destructuring at call
/// sites.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OciDirRef {
    pub manifest_digest: String,
    pub image_name: Option<ImageName>,
}

/// Outcome of importing a single OCI Image Layout directory into the
/// v3 SQLite Local Registry.
///
/// `manifest_digest` and `image_name` carry the same identity that
/// [`OciDirRef`] would have reported for the source directory (import
/// is identity-preserving — see [`import_oci_dir`]).
///
/// `ref_update` distinguishes the four outcomes the SQLite ref
/// transition can take:
///
/// - `Some(Inserted)` — fresh ref published.
/// - `Some(Unchanged)` — same `image_name` already pointed at the same
///   manifest digest; the import was an idempotent verify.
/// - `Some(Replaced { previous_manifest_digest })` — caller passed
///   `RefConflictPolicy::Replace` and an older digest was overwritten.
/// - `Some(Conflicted { existing, incoming })` — only seen by callers
///   that drive the import with `RefConflictHandling::Return` (e.g.
///   the legacy batch importer); the public functions still surface a
///   conflict as `Result::Err`.
/// - `None` — the OCI dir had no `org.opencontainers.image.ref.name`
///   annotation and was imported by digest only, so no SQLite ref was
///   set.
///
/// `#[non_exhaustive]` so future fields (for example, the number of
/// blobs newly written versus reused via CAS dedup) are additive.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OciDirImport {
    pub manifest_digest: String,
    pub image_name: Option<ImageName>,
    pub ref_update: Option<RefUpdate>,
}

impl OciDirImport {
    fn from_inner(ref_info: OciDirRef, ref_update: Option<RefUpdate>) -> Self {
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

/// All the read-only state that a single OCI Image Layout directory
/// contributes to a v3 import: identity (digest + ref-name annotation),
/// the manifest bytes / descriptor that get persisted verbatim, the
/// format tag, the layer descriptors enumerated from the manifest,
/// and (Image Manifest only) the config blob.
///
/// "Staged" parallels the build-side vocabulary
/// ([`crate::artifact::StagedArtifactBlob`],
/// [`crate::artifact::LocalArtifactBuilder::stage`]): the data is
/// fully computed and ready for publish, but the IndexStore writes
/// have not happened yet.
///
/// Built once by [`stage_oci_dir`] and consumed by
/// [`import_oci_dir_inner`]. v3 import is identity-preserving:
/// `manifest_bytes` and `manifest_digest` are stored verbatim. The only
/// supported manifest format is OCI Image Manifest (with `artifactType`
/// set to the OMMX artifact media type); the deprecated OCI Artifact
/// Manifest is rejected at parse time.
struct StagedOciDir {
    manifest_digest: String,
    image_name: Option<ImageName>,
    manifest_bytes: Vec<u8>,
    manifest_descriptor: Descriptor,
    layers: Vec<Descriptor>,
    annotations: Option<HashMap<String, String>>,
    subject: Option<Descriptor>,
    image_config: (Descriptor, Vec<u8>),
}

/// Import a standard OCI Image Layout directory into the v3 local registry.
///
/// Works for any OCI Image Layout (`oci-layout` + `index.json` + `blobs/`),
/// regardless of how it was produced. The v3 registry does not keep
/// `index.json` as mutable state; it only reads it to discover the single
/// manifest, and then copies the exact content-addressed blobs into
/// [`FileBlobStore`] while inserting the matching records into
/// [`SqliteIndexStore`]. The manifest digest is preserved verbatim — import
/// never rewrites the manifest. Reformatting an Image Manifest into an
/// Artifact Manifest is a separate explicit `convert` operation that
/// produces a new artifact under a new digest / new ref, intentionally not
/// a side effect of import.
pub fn import_oci_dir(
    index_store: &SqliteIndexStore,
    blob_store: &FileBlobStore,
    oci_dir_root: impl AsRef<Path>,
) -> Result<OciDirImport> {
    import_oci_dir_with_policy(
        index_store,
        blob_store,
        oci_dir_root,
        RefConflictPolicy::KeepExisting,
    )
}

pub fn import_oci_dir_with_policy(
    index_store: &SqliteIndexStore,
    blob_store: &FileBlobStore,
    oci_dir_root: impl AsRef<Path>,
    policy: RefConflictPolicy,
) -> Result<OciDirImport> {
    let (ref_info, ref_update) = import_oci_dir_inner(
        index_store,
        blob_store,
        oci_dir_root,
        None,
        policy,
        RefConflictHandling::Error,
    )?;
    Ok(OciDirImport::from_inner(ref_info, ref_update))
}

/// Unified inner implementation used by every public `import_oci_dir*`
/// entry point and by the v2 batch import in [`super::legacy`].
///
/// `target_image_name` controls which SQLite ref the import writes:
///
/// - `None` — fall back to the OCI dir's
///   `org.opencontainers.image.ref.name` annotation. If the dir has
///   no such annotation, no ref is written and the artifact is
///   reachable by digest only.
/// - `Some(name)` — always publish under `name`. If the dir also has
///   an annotation, the two are checked for equality and a mismatch
///   is an error.
///
/// The returned `OciDirRef.image_name` reflects the **effective ref
/// actually written**, not just the source annotation, so callers
/// that pass `target_image_name` for an unannotated dir still see the
/// ref they published in the result.
pub(super) fn import_oci_dir_inner(
    index_store: &SqliteIndexStore,
    blob_store: &FileBlobStore,
    oci_dir_root: impl AsRef<Path>,
    target_image_name: Option<&ImageName>,
    policy: RefConflictPolicy,
    conflict_handling: RefConflictHandling,
) -> Result<(OciDirRef, Option<RefUpdate>)> {
    let oci_dir_root = oci_dir_root.as_ref();
    let staged = stage_oci_dir(oci_dir_root)?;
    let manifest_digest = staged.manifest_digest.clone();
    if let (Some(target), Some(annotated)) = (target_image_name, staged.image_name.as_ref()) {
        ensure!(
            target == annotated,
            "OCI dir ref mismatch: requested={}, annotated={}",
            target,
            annotated
        );
    }
    // The ref the caller wants written takes precedence over the
    // annotation; if neither is provided, no ref is written.
    let effective_image_name = target_image_name
        .cloned()
        .or_else(|| staged.image_name.clone());

    // Pre-check: under `KeepExisting`, surface the conflict before we
    // stage any CAS writes. The atomic publish in stage 2 re-validates
    // the same condition inside the SQLite transaction, so concurrent
    // writers still get a consistent outcome; this is purely a fast
    // path for the common single-writer case.
    if policy == RefConflictPolicy::KeepExisting {
        if let Some(image_name) = effective_image_name.as_ref() {
            if let Some(existing_manifest_digest) = index_store.resolve_image_name(image_name)? {
                if existing_manifest_digest != manifest_digest {
                    if conflict_handling == RefConflictHandling::Error {
                        anyhow::bail!(
                            "Local registry ref conflict for {}: existing manifest {}, incoming manifest {}",
                            image_name,
                            existing_manifest_digest,
                            manifest_digest,
                        );
                    }
                    let conflict = RefUpdate::Conflicted {
                        existing_manifest_digest,
                        incoming_manifest_digest: manifest_digest.clone(),
                    };
                    return Ok((
                        OciDirRef {
                            manifest_digest,
                            image_name: effective_image_name,
                        },
                        Some(conflict),
                    ));
                }
            }
        }
    }

    // Stage 1: write CAS bytes for layers, optional config, and the
    // manifest itself. These are idempotent so they don't need to
    // share a SQLite transaction.
    let layer_descriptors = staged.layers.as_slice();
    let mut blob_records = Vec::with_capacity(layer_descriptors.len() + 2);
    let mut layer_records = Vec::with_capacity(layer_descriptors.len());
    for (position, layer) in layer_descriptors.iter().enumerate() {
        let record =
            stage_oci_dir_descriptor_blob(blob_store, oci_dir_root, layer, BLOB_KIND_BLOB)?;
        blob_records.push(record);
        layer_records.push(LayerRecord {
            manifest_digest: manifest_digest.clone(),
            position: u32::try_from(position).context("Layer position does not fit in u32")?,
            digest: digest_to_string(layer.digest()),
            media_type: layer.media_type().to_string(),
            size: layer.size(),
            annotations_json: annotations_json(layer.annotations().as_ref())?,
        });
    }
    let (config_descriptor, config_bytes) = &staged.image_config;
    blob_records.push(stage_descriptor_bytes(
        blob_store,
        config_descriptor,
        config_bytes,
        BLOB_KIND_CONFIG,
    )?);
    blob_records.push(stage_descriptor_bytes(
        blob_store,
        &staged.manifest_descriptor,
        &staged.manifest_bytes,
        BLOB_KIND_MANIFEST,
    )?);

    let manifest_record = ManifestRecord {
        digest: manifest_digest.clone(),
        media_type: OCI_IMAGE_MANIFEST_MEDIA_TYPE.to_string(),
        size: staged.manifest_descriptor.size(),
        subject_digest: staged
            .subject
            .as_ref()
            .map(|d| digest_to_string(d.digest())),
        annotations_json: annotations_json(staged.annotations.as_ref())?,
        created_at: now_rfc3339(),
    };

    // Stage 2: one SQLite transaction commits blob records + manifest
    // + ref. A crash or conflict between stages can never leave
    // committed manifest / blob rows under a ref the caller did not
    // actually publish.
    let outcome = index_store.publish_artifact_atomic(
        &blob_records,
        &manifest_record,
        &layer_records,
        effective_image_name.as_ref(),
        policy,
    )?;
    let ref_update = match outcome.ref_update {
        Some(RefUpdate::Conflicted {
            existing_manifest_digest,
            incoming_manifest_digest,
        }) if conflict_handling == RefConflictHandling::Error => {
            anyhow::bail!(
                "Local registry ref conflict for {}: existing manifest {}, incoming manifest {}",
                effective_image_name
                    .as_ref()
                    .expect("Conflicted requires an effective image_name"),
                existing_manifest_digest,
                incoming_manifest_digest
            )
        }
        update => update,
    };

    Ok((
        OciDirRef {
            manifest_digest,
            image_name: effective_image_name,
        },
        ref_update,
    ))
}

pub fn import_oci_dir_as_ref(
    index_store: &SqliteIndexStore,
    blob_store: &FileBlobStore,
    oci_dir_root: impl AsRef<Path>,
    image_name: &ImageName,
) -> Result<OciDirImport> {
    import_oci_dir_as_ref_with_policy(
        index_store,
        blob_store,
        oci_dir_root,
        image_name,
        RefConflictPolicy::KeepExisting,
    )
}

pub fn import_oci_dir_as_ref_with_policy(
    index_store: &SqliteIndexStore,
    blob_store: &FileBlobStore,
    oci_dir_root: impl AsRef<Path>,
    image_name: &ImageName,
    policy: RefConflictPolicy,
) -> Result<OciDirImport> {
    let (ref_info, ref_update) = import_oci_dir_inner(
        index_store,
        blob_store,
        oci_dir_root,
        Some(image_name),
        policy,
        RefConflictHandling::Error,
    )?;
    Ok(OciDirImport::from_inner(ref_info, ref_update))
}

pub fn oci_dir_image_name(oci_dir_root: impl AsRef<Path>) -> Result<Option<ImageName>> {
    Ok(oci_dir_ref(oci_dir_root)?.image_name)
}

pub fn oci_dir_ref(oci_dir_root: impl AsRef<Path>) -> Result<OciDirRef> {
    let staged = stage_oci_dir(oci_dir_root)?;
    Ok(OciDirRef {
        manifest_digest: staged.manifest_digest,
        image_name: staged.image_name,
    })
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

fn stage_oci_dir(oci_dir_root: impl AsRef<Path>) -> Result<StagedOciDir> {
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
    let manifest_digest = digest_to_string(index_descriptor.digest());
    let manifest_bytes = read_oci_dir_blob(oci_dir_root, &manifest_digest)
        .with_context(|| format!("Failed to read manifest blob {manifest_digest}"))?;
    ensure!(
        manifest_bytes.len() as u64 == index_descriptor.size(),
        "Manifest blob size mismatch for {manifest_digest}: descriptor={}, actual={}",
        index_descriptor.size(),
        manifest_bytes.len()
    );

    // v3 supports only OCI Image Manifest. The deprecated OCI Artifact
    // Manifest is rejected at parse time.
    let (layers, annotations, subject, image_config) = match index_descriptor.media_type() {
        MediaType::ImageManifest => stage_image_manifest(oci_dir_root, &manifest_bytes)?,
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

    let manifest_digest_parsed = Digest::from_str(&manifest_digest)
        .with_context(|| format!("Invalid manifest digest: {manifest_digest}"))?;
    let manifest_descriptor = DescriptorBuilder::default()
        .media_type(MediaType::ImageManifest)
        .digest(manifest_digest_parsed)
        .size(manifest_bytes.len() as u64)
        .build()
        .context("Failed to build OCI manifest descriptor")?;

    Ok(StagedOciDir {
        manifest_digest,
        image_name,
        manifest_bytes,
        manifest_descriptor,
        layers,
        annotations,
        subject,
        image_config,
    })
}

/// Manifest-derived fields filled into [`StagedOciDir`] by `stage_oci_dir`.
type StagedManifestFields = (
    Vec<Descriptor>,
    Option<HashMap<String, String>>,
    Option<Descriptor>,
    (Descriptor, Vec<u8>),
);

fn stage_image_manifest(
    oci_dir_root: &Path,
    manifest_bytes: &[u8],
) -> Result<StagedManifestFields> {
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
        manifest.annotations().clone(),
        manifest.subject().clone(),
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

/// Read a layer / config blob out of the legacy OCI dir, write it to
/// the v3 [`FileBlobStore`] under its content digest, and return the
/// matching [`BlobRecord`] for the IndexStore. The DB row is *not*
/// inserted here — the caller hands the records to
/// [`SqliteIndexStore::publish_artifact_atomic`] so all blob /
/// manifest / ref inserts share one transaction.
fn stage_oci_dir_descriptor_blob(
    blob_store: &FileBlobStore,
    oci_dir_root: &Path,
    desc: &Descriptor,
    kind: &str,
) -> Result<BlobRecord> {
    let digest = digest_to_string(desc.digest());
    let bytes = read_oci_dir_blob(oci_dir_root, &digest)
        .with_context(|| format!("Failed to read {kind} blob {digest}"))?;
    ensure!(
        bytes.len() as u64 == desc.size(),
        "{kind} blob size mismatch for {digest}: descriptor={}, actual={}",
        desc.size(),
        bytes.len()
    );
    stage_descriptor_bytes(blob_store, desc, &bytes, kind)
}

/// CAS-write `bytes` for `desc` and return the matching
/// [`BlobRecord`] (no DB write).
fn stage_descriptor_bytes(
    blob_store: &FileBlobStore,
    desc: &Descriptor,
    bytes: &[u8],
    kind: &str,
) -> Result<BlobRecord> {
    let mut record = blob_store.put_bytes(bytes)?;
    ensure!(
        record.digest == digest_to_string(desc.digest()),
        "{kind} blob digest mismatch: descriptor={}, actual={}",
        desc.digest(),
        record.digest
    );
    ensure!(
        record.size == desc.size(),
        "{kind} blob size mismatch for {}: descriptor={}, actual={}",
        desc.digest(),
        desc.size(),
        record.size
    );
    record.media_type = Some(desc.media_type().to_string());
    record.kind = kind.to_string();
    Ok(record)
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

fn image_name_from_index_descriptor(desc: &Descriptor) -> Result<Option<ImageName>> {
    desc.annotations()
        .as_ref()
        .and_then(|annotations| annotations.get(OCI_IMAGE_REF_NAME_ANNOTATION))
        .map(|name| ImageName::parse(name).with_context(|| format!("Invalid image ref: {name}")))
        .transpose()
}

fn digest_to_string<D: std::fmt::Display + ?Sized>(digest: &D) -> String {
    digest.to_string()
}
