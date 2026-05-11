//! Remote OCI registry → v3 SQLite Local Registry import.
//!
//! ## Naming note: `pull_image` vs `import_*`
//!
//! The other import sources expose `import_<noun>` entry points
//! (`import_oci_dir`, `import_oci_archive`, `import_legacy_local_registry`).
//! This module deliberately names its entry point [`pull_image`]
//! instead, mirroring the OCI Distribution Spec verb and the
//! surrounding ecosystem (`docker pull`, `oras pull`, `crane pull`).
//! Renaming it to `import_remote` would lose the OCI-domain signal
//! that the operation is a network fetch with the standard pull
//! semantics; the `import` namespace it lives in already conveys
//! that the result lands in the v3 registry.
//!
//! ## Implementation shape
//!
//! Step D (§12.4) replaced the previous "remote → temp OCI Image Layout
//! → import_oci_dir" two-stage pipeline with a single network-to-SQLite
//! pass through [`super::super::super::remote_transport::RemoteTransport`]:
//!
//! 1. Pre-pull SQLite check short-circuits the network fetch when the
//!    registry already resolves `image_name` to a manifest digest **and**
//!    the manifest blob is present in [`super::super::FileBlobStore`].
//!    The function returns an [`OciDirImport`] with
//!    [`super::super::RefUpdate::Unchanged`] without touching the
//!    network. The blob-presence probe distinguishes a healthy hit from
//!    a half-populated registry (e.g. manual blob-store deletion,
//!    interrupted import): the latter falls through to a fresh pull
//!    with a `tracing::warn!` event rather than handing back a stale
//!    `Unchanged` that would surface as an opaque `get_blob` failure
//!    later. Same cache-hit semantics the v2-era legacy dir cache
//!    offered, expressed against the canonical SQLite ref store.
//! 2. Open a [`RemoteTransport`], authenticate for `Pull`, fetch the
//!    manifest bytes verbatim, then walk the manifest's config +
//!    layer descriptors. Each blob is pulled into memory, written to
//!    [`FileBlobStore`], and the matching [`BlobRecord`] / [`LayerRecord`]
//!    is staged. The manifest is staged last so it sits behind its
//!    blobs in the BlobStore (matching the OCI distribution publish
//!    order).
//! 3. One SQLite transaction (`publish_artifact_atomic`) commits every
//!    [`BlobRecord`] + the [`ManifestRecord`] + the ref update under
//!    the requested `image_name`. A crash between blob writes and the
//!    publish leaves orphan CAS bytes (recovered by GC, not visible
//!    through the index); a crash inside the SQLite transaction never
//!    leaves a partially-published ref. Concurrent first-miss pulls
//!    for the same image converge inside this transaction.
//!
//! v3 has no on-disk OCI Image Layout intermediate for pulls — SQLite
//! plus [`FileBlobStore`] are the sole post-import home of the bytes.
//!
//! Feature-gated behind `remote-artifact` because the [`RemoteTransport`]
//! is, and because this is the only place in `local_registry` that
//! touches the network.

use super::super::{
    annotations_json, now_rfc3339, BlobRecord, FileBlobStore, LayerRecord, LocalRegistry,
    ManifestRecord, RefConflictPolicy, RefUpdate, BLOB_KIND_BLOB, BLOB_KIND_CONFIG,
    BLOB_KIND_MANIFEST,
};
use super::oci_dir::OciDirImport;
use crate::artifact::{
    media_types, remote_transport::RemoteTransport, OCI_IMAGE_MANIFEST_MEDIA_TYPE,
};
use anyhow::{Context, Result};
use oci_client::RegistryOperation;
use oci_spec::image::{Descriptor, ImageManifest, MediaType};
use ocipkg::ImageName;
use std::sync::Arc;

/// Pull `image_name` from its remote registry into the v3 SQLite
/// Local Registry.
///
/// If the registry already resolves `image_name` to a manifest digest
/// whose blob is present in the `FileBlobStore`, the network fetch is
/// skipped and the function returns an [`OciDirImport`] with
/// [`RefUpdate::Unchanged`]. If the ref resolves but the manifest blob
/// is missing (registry corruption, interrupted import, manual blob
/// deletion), the function logs a `tracing::warn!` and falls through
/// to a fresh pull rather than handing back a stale `Unchanged` — that
/// would surface later as an opaque `get_blob` failure with no
/// recovery hint. Layer-blob completeness is not probed: if the
/// manifest is present, layers are assumed to follow from the same
/// publish transaction (`publish_artifact_atomic`); a layer-only gap
/// is a strict registry-corruption case and out of scope for this
/// fast path.
///
/// Otherwise the manifest and each blob are pulled through
/// [`RemoteTransport`] straight into [`FileBlobStore`], and a single
/// SQLite transaction publishes the ref. There is no on-disk OCI Image
/// Layout intermediate.
///
/// Concurrent first-miss pulls for the same image race at the SQLite
/// `publish_artifact_atomic` boundary. **Assuming the remote registry
/// returns byte-identical manifests across both requests**, the second
/// writer sees `Unchanged`. If the remote serves non-deterministic
/// manifest bytes (field reorder, whitespace drift) the two digests
/// differ and the loser surfaces a `Conflicted` outcome under
/// `KeepExisting`; callers that need last-writer-wins semantics in
/// that case should drive the import with `RefConflictPolicy::Replace`.
pub fn pull_image(registry: &Arc<LocalRegistry>, image_name: &ImageName) -> Result<OciDirImport> {
    if let Some(manifest_digest) = registry.index().resolve_image_name(image_name)? {
        if registry.blobs().exists(&manifest_digest)? {
            return Ok(OciDirImport {
                manifest_digest,
                image_name: Some(image_name.clone()),
                ref_update: Some(RefUpdate::Unchanged),
            });
        }
        tracing::warn!(
            "SQLite ref resolves {image_name} → {manifest_digest}, but the manifest \
             blob is missing from FileBlobStore; falling through to a fresh remote \
             pull to repopulate the registry",
        );
    }

    let transport = RemoteTransport::new(image_name)?;
    transport.auth_for(image_name, RegistryOperation::Pull)?;

    tracing::info!("Pulling {image_name} into the v3 Local Registry");
    let (manifest_bytes, manifest_digest) = transport.pull_manifest_raw(
        image_name,
        &[
            OCI_IMAGE_MANIFEST_MEDIA_TYPE,
            "application/vnd.oci.image.index.v1+json",
        ],
    )?;
    let manifest: ImageManifest = serde_json::from_slice(&manifest_bytes)
        .context("Failed to parse OCI image manifest pulled from the remote registry")?;
    ensure_ommx_image_manifest(&manifest)?;

    let blob_count = manifest.layers().len() + 2;
    let mut blob_records = Vec::with_capacity(blob_count);
    let mut layer_records = Vec::with_capacity(manifest.layers().len());

    let config_descriptor = manifest.config();
    blob_records.push(pull_descriptor_blob(
        &transport,
        registry.blobs(),
        image_name,
        config_descriptor,
        BLOB_KIND_CONFIG,
    )?);

    for (position, layer) in manifest.layers().iter().enumerate() {
        blob_records.push(pull_descriptor_blob(
            &transport,
            registry.blobs(),
            image_name,
            layer,
            BLOB_KIND_BLOB,
        )?);
        layer_records.push(LayerRecord {
            manifest_digest: manifest_digest.clone(),
            position: u32::try_from(position).context("Layer position does not fit in u32")?,
            digest: layer.digest().to_string(),
            media_type: layer.media_type().to_string(),
            size: layer.size(),
            annotations_json: annotations_json(layer.annotations().as_ref())?,
        });
    }

    blob_records.push(stage_manifest_blob(
        registry.blobs(),
        &manifest_bytes,
        &manifest_digest,
    )?);

    let manifest_record = ManifestRecord {
        digest: manifest_digest.clone(),
        media_type: OCI_IMAGE_MANIFEST_MEDIA_TYPE.to_string(),
        size: manifest_bytes.len() as u64,
        subject_digest: manifest
            .subject()
            .as_ref()
            .map(|subject| subject.digest().to_string()),
        annotations_json: annotations_json(manifest.annotations().as_ref())?,
        created_at: now_rfc3339(),
    };

    let outcome = registry.index().publish_artifact_atomic(
        &blob_records,
        &manifest_record,
        &layer_records,
        Some(image_name),
        RefConflictPolicy::KeepExisting,
    )?;
    // Surface a ref conflict as `Err` rather than `Ok(Conflicted)`:
    // callers (Python `Artifact.load`, CLI `ommx pull`, dataset
    // loaders) treat a successful return as "the freshly pulled bytes
    // are now resident under `image_name`". Under `KeepExisting`, a
    // conflict means the SQLite ref still points at the *prior*
    // manifest digest; opening `LocalArtifact` after that would
    // silently surface the local cache, not the remote bytes. Forcing
    // an explicit error lets callers decide between `--replace`
    // semantics and aborting.
    if let Some(RefUpdate::Conflicted {
        existing_manifest_digest,
        incoming_manifest_digest,
    }) = &outcome.ref_update
    {
        anyhow::bail!(
            "Local registry ref conflict for {image_name}: existing manifest \
             {existing_manifest_digest}, incoming manifest {incoming_manifest_digest}. \
             The remote serves a different manifest than the one cached locally; \
             retry with a replace policy if you want to overwrite the local ref."
        );
    }

    Ok(OciDirImport {
        manifest_digest,
        image_name: Some(image_name.clone()),
        ref_update: outcome.ref_update,
    })
}

/// Validate that the remote manifest is an OCI Image Manifest carrying
/// the OMMX `artifactType` field. v3 SDK accepts only this format;
/// callers that need to publish other artifacts go through the OCI
/// distribution APIs directly.
fn ensure_ommx_image_manifest(manifest: &ImageManifest) -> Result<()> {
    let artifact_type = manifest
        .artifact_type()
        .as_ref()
        .context("Remote manifest is not an OMMX artifact: artifactType is missing")?;
    anyhow::ensure!(
        artifact_type == &media_types::v1_artifact(),
        "Remote manifest is not an OMMX artifact: {artifact_type}"
    );
    if let Some(media_type) = manifest.media_type() {
        anyhow::ensure!(
            media_type == &MediaType::ImageManifest,
            "Remote manifest media type must be OCI Image Manifest, got {media_type}"
        );
    }
    Ok(())
}

/// Pull a single descriptor's blob from the registry, write it into
/// [`FileBlobStore`] under its content digest, and produce the matching
/// [`BlobRecord`]. The DB row is *not* inserted here — the caller hands
/// the record to [`SqliteIndexStore::publish_artifact_atomic`] so blob /
/// manifest / ref inserts share one transaction.
fn pull_descriptor_blob(
    transport: &RemoteTransport,
    blob_store: &FileBlobStore,
    image_name: &ImageName,
    descriptor: &Descriptor,
    kind: &str,
) -> Result<BlobRecord> {
    let digest = descriptor.digest().to_string();
    // The manifest descriptor's `size` bounds the network read: the
    // transport's pull helper allocates from this value (not from the
    // registry-reported `Content-Length`) and aborts the chunk loop if
    // the registry serves more bytes than declared.
    let bytes = transport.pull_blob_to_vec(image_name, &digest, descriptor.size())?;
    anyhow::ensure!(
        bytes.len() as u64 == descriptor.size(),
        "{kind} blob size mismatch for {digest}: descriptor={}, actual={}",
        descriptor.size(),
        bytes.len()
    );
    let mut record = blob_store.put_bytes(&bytes)?;
    anyhow::ensure!(
        record.digest == digest,
        "{kind} blob digest mismatch: descriptor={digest}, actual={}",
        record.digest
    );
    record.media_type = Some(descriptor.media_type().to_string());
    record.kind = kind.to_string();
    Ok(record)
}

/// Stage the manifest bytes into [`FileBlobStore`] under their
/// registry-reported digest. The check that local sha256 matches the
/// registry-reported digest doubles as an integrity probe on the
/// manifest body: an upstream proxy that rewrote the manifest would
/// surface here instead of producing an artifact whose published ref
/// points at a manifest blob the registry does not actually serve.
fn stage_manifest_blob(
    blob_store: &FileBlobStore,
    manifest_bytes: &[u8],
    expected_digest: &str,
) -> Result<BlobRecord> {
    let mut record = blob_store.put_bytes(manifest_bytes)?;
    anyhow::ensure!(
        record.digest == expected_digest,
        "Manifest blob digest mismatch: registry reported {expected_digest}, sha256 of \
         pulled bytes is {}",
        record.digest
    );
    record.media_type = Some(OCI_IMAGE_MANIFEST_MEDIA_TYPE.to_string());
    record.kind = BLOB_KIND_MANIFEST.to_string();
    Ok(record)
}
