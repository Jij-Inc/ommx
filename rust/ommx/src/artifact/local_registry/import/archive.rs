//! `.ommx` OCI archive → v3 SQLite Local Registry import.
//!
//! A native tar reader writes blobs straight into
//! [`super::super::FileBlobStore`] and publishes the manifest + ref
//! atomically through [`super::super::SqliteIndexStore`]. There is no
//! on-disk OCI Image Layout intermediate — SQLite + `FileBlobStore`
//! are the sole post-import home of the bytes.
//!
//! The reader streams the archive once. For each tar entry it
//! classifies on path:
//!
//! - `oci-layout` — JSON file, version `1.0.0` enforced; other
//!   versions are rejected as unsupported.
//! - `index.json` — JSON file describing a single-entry `ImageIndex`;
//!   captured into memory for post-pass parsing.
//! - `blobs/<algorithm>/<encoded>` — content-addressed blob, written
//!   straight into [`super::super::FileBlobStore`] via `put_bytes`
//!   which re-derives sha256 and asserts the result matches the
//!   tar-entry path encoding.
//! - Anything else — silently ignored to stay forwards-compatible with
//!   OCI Image Layout extensions that may add sibling files in the
//!   archive root (signatures, signatures-of-signatures, etc.).
//!
//! After the pass: parse `index.json`, locate the named manifest in
//! [`super::super::FileBlobStore`] by digest, parse the manifest, and
//! emit one [`super::super::SqliteIndexStore::publish_artifact_atomic`]
//! call with the manifest / config / layer records under the
//! `org.opencontainers.image.ref.name` annotated ref. A crash between
//! blob writes and publish leaves orphan CAS bytes recoverable by GC;
//! a crash inside the SQLite transaction never leaves a partially-
//! published ref.

use super::super::{
    annotations_json, now_rfc3339, sha256_digest, BlobRecord, FileBlobStore, LayerRecord,
    LocalRegistry, ManifestRecord, RefConflictPolicy, RefUpdate, ValidatedDigest, BLOB_KIND_BLOB,
    BLOB_KIND_CONFIG, BLOB_KIND_MANIFEST, OCI_IMAGE_REF_NAME_ANNOTATION,
};
use super::oci_dir::OciDirImport;
use crate::artifact::{media_types, OCI_IMAGE_MANIFEST_MEDIA_TYPE};
use anyhow::{Context, Result};
use oci_spec::image::{Descriptor, ImageIndex, ImageManifest, MediaType, OciLayout};
use ocipkg::ImageName;
use std::{
    fs::File,
    io::{BufReader, Read},
    path::Path,
    sync::Arc,
};
use tar::Archive;

/// Read just the OCI image manifest out of a `.ommx` archive without
/// importing it into the SQLite Local Registry. Used by CLI
/// `ommx inspect <archive>` to surface the manifest as JSON without
/// the side effect of populating the user's registry. Streams the tar
/// once: extracts `index.json` to locate the manifest descriptor, then
/// re-opens the archive and walks tar entries to find the manifest
/// blob by digest (manifest first in the v3 native writer's output, so
/// the second pass terminates quickly for archives produced by v3).
pub fn read_archive_manifest(path: &Path) -> Result<ImageManifest> {
    let manifest_digest = read_archive_manifest_digest(path)?;

    let file = File::open(path)
        .with_context(|| format!("Failed to open OCI archive {}", path.display()))?;
    let mut archive = Archive::new(BufReader::new(file));
    for entry in archive
        .entries()
        .with_context(|| format!("Failed to read tar entries in {}", path.display()))?
    {
        let mut entry =
            entry.with_context(|| format!("Failed to read tar entry in {}", path.display()))?;
        let entry_path = entry
            .path()
            .with_context(|| format!("Failed to decode tar entry path in {}", path.display()))?
            .into_owned();
        let path_str = entry_path.to_string_lossy();
        if !matches!(entry.header().entry_type(), tar::EntryType::Regular) {
            continue;
        }
        if let Some(digest) = blob_path_to_digest(&path_str) {
            if digest == manifest_digest {
                let mut bytes = Vec::with_capacity(entry.header().size().unwrap_or(0) as usize);
                entry.read_to_end(&mut bytes).with_context(|| {
                    format!(
                        "Failed to read manifest blob {manifest_digest} from {}",
                        path.display()
                    )
                })?;
                anyhow::ensure!(
                    sha256_digest(&bytes) == manifest_digest,
                    "Manifest blob {manifest_digest} in {} fails sha256 check",
                    path.display()
                );
                let manifest: ImageManifest =
                    serde_json::from_slice(&bytes).with_context(|| {
                        format!(
                            "Failed to parse OCI image manifest blob {manifest_digest} in {}",
                            path.display()
                        )
                    })?;
                return Ok(manifest);
            }
        }
    }
    anyhow::bail!(
        "Manifest blob {manifest_digest} declared by index.json is missing from archive {}",
        path.display()
    );
}

/// First-pass helper for [`read_archive_manifest`]: stream the tar to
/// find `index.json` and return the manifest digest it points at.
fn read_archive_manifest_digest(path: &Path) -> Result<String> {
    let file = File::open(path)
        .with_context(|| format!("Failed to open OCI archive {}", path.display()))?;
    let mut archive = Archive::new(BufReader::new(file));
    for entry in archive
        .entries()
        .with_context(|| format!("Failed to read tar entries in {}", path.display()))?
    {
        let mut entry =
            entry.with_context(|| format!("Failed to read tar entry in {}", path.display()))?;
        let entry_path = entry
            .path()
            .with_context(|| format!("Failed to decode tar entry path in {}", path.display()))?
            .into_owned();
        if !matches!(entry.header().entry_type(), tar::EntryType::Regular) {
            continue;
        }
        if entry_path.to_string_lossy() != "index.json" {
            continue;
        }
        let mut bytes = Vec::with_capacity(entry.header().size().unwrap_or(0) as usize);
        entry
            .read_to_end(&mut bytes)
            .with_context(|| format!("Failed to read index.json from {}", path.display()))?;
        let image_index: ImageIndex = serde_json::from_slice(&bytes)
            .with_context(|| format!("Failed to parse index.json in {}", path.display()))?;
        anyhow::ensure!(
            image_index.manifests().len() == 1,
            "OMMX OCI archive must contain exactly one manifest in index.json: {}",
            path.display()
        );
        return Ok(image_index
            .manifests()
            .first()
            .unwrap()
            .digest()
            .to_string());
    }
    anyhow::bail!("Missing index.json in {}", path.display())
}

/// Import a `.ommx` OCI archive on disk into the v3 SQLite Local Registry.
///
/// Streams the archive entries once: writes every `blobs/sha256/<digest>`
/// blob straight into [`FileBlobStore`] (which re-derives sha256 and
/// asserts the recomputed digest matches the tar path), captures
/// `oci-layout` + `index.json` into memory for the post-pass parse,
/// and finally emits a single SQLite transaction that publishes the
/// manifest + ref. Returns the [`OciDirImport`] outcome reported by
/// the underlying publish (`Inserted` on first call for this image,
/// `Unchanged` for an idempotent re-import of the same digest, or
/// `Err` for a ref conflict when the new archive's manifest digest
/// differs from the SQLite-recorded one under `KeepExisting` policy).
pub fn import_oci_archive(registry: &Arc<LocalRegistry>, path: &Path) -> Result<OciDirImport> {
    let file = File::open(path)
        .with_context(|| format!("Failed to open OCI archive {}", path.display()))?;
    let scanned = scan_archive(BufReader::new(file), registry.blobs(), path)?;

    // The OCI Image Layout spec requires `oci-layout`, but a number of
    // historical tools — including the v2-era OMMX SDK's
    // `OciArchiveBuilder` — omit it and still produce otherwise
    // spec-compliant archives. Accept both shapes: validate the
    // version when present, warn-and-proceed when absent. `index.json`
    // is non-negotiable; an archive without it is not addressable.
    match scanned.oci_layout.as_deref() {
        Some(bytes) => {
            let oci_layout: OciLayout = serde_json::from_slice(bytes)
                .with_context(|| format!("Failed to parse oci-layout in {}", path.display()))?;
            anyhow::ensure!(
                oci_layout.image_layout_version() == "1.0.0",
                "Unsupported OCI layout version in {}: {}",
                path.display(),
                oci_layout.image_layout_version()
            );
        }
        None => {
            tracing::warn!(
                "{} has no oci-layout marker; assuming OCI Image Layout 1.0.0 \
                 (matches archives produced by pre-v3 OMMX / older oras / crane)",
                path.display()
            );
        }
    }
    let index_bytes = scanned
        .index_json
        .with_context(|| format!("Missing index.json in {}", path.display()))?;
    let image_index: ImageIndex = serde_json::from_slice(&index_bytes)
        .with_context(|| format!("Failed to parse index.json in {}", path.display()))?;
    anyhow::ensure!(
        image_index.manifests().len() == 1,
        "OMMX OCI archive must contain exactly one manifest in index.json: {}",
        path.display()
    );
    let index_descriptor = image_index.manifests().first().unwrap();
    let image_name = image_name_from_index_descriptor(index_descriptor)?;
    let manifest_digest = index_descriptor.digest().to_string();

    // The manifest blob is now resident in the BlobStore (it was a
    // `blobs/sha256/<digest>` entry in the tar). Read it back to
    // discover the layers; this re-verifies its digest in the process.
    let manifest_bytes = registry
        .blobs()
        .read_bytes(&manifest_digest)
        .with_context(|| {
            format!(
                "Manifest blob {manifest_digest} declared in index.json is missing from \
                 the archive at {}",
                path.display()
            )
        })?;
    anyhow::ensure!(
        manifest_bytes.len() as u64 == index_descriptor.size(),
        "Manifest blob size mismatch in {}: index.json claims {}, blob is {} bytes",
        path.display(),
        index_descriptor.size(),
        manifest_bytes.len()
    );
    let media_type = match index_descriptor.media_type() {
        MediaType::ImageManifest => MediaType::ImageManifest,
        MediaType::ArtifactManifest => anyhow::bail!(
            "OCI archive in {} uses the deprecated OCI Artifact Manifest \
             (application/vnd.oci.artifact.manifest.v1+json), which is not supported. \
             v3 OMMX accepts only OCI Image Manifest with artifactType.",
            path.display()
        ),
        other => anyhow::bail!(
            "OCI archive in {} has unsupported manifest media type {other}; expected \
             OMMX Image Manifest.",
            path.display()
        ),
    };
    let manifest: ImageManifest = serde_json::from_slice(&manifest_bytes)
        .with_context(|| format!("Failed to parse OCI image manifest in {}", path.display()))?;
    ensure_ommx_artifact_type(manifest.artifact_type().as_ref())?;

    // Build the BlobRecord / LayerRecord / ManifestRecord trio. All
    // referenced blobs are already in `FileBlobStore` (we wrote them
    // during the tar scan); we just need to surface the records the
    // SQLite publish wants.
    let layer_count = manifest.layers().len();
    let mut blob_records = Vec::with_capacity(layer_count + 2);
    let mut layer_records = Vec::with_capacity(layer_count);

    blob_records.push(record_for_blob(
        registry.blobs(),
        manifest.config(),
        BLOB_KIND_CONFIG,
        path,
    )?);
    for (position, layer) in manifest.layers().iter().enumerate() {
        blob_records.push(record_for_blob(
            registry.blobs(),
            layer,
            BLOB_KIND_BLOB,
            path,
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
    let manifest_record_blob = record_for_manifest_blob(
        registry.blobs(),
        &manifest_digest,
        manifest_bytes.len() as u64,
    )?;
    blob_records.push(manifest_record_blob);

    let manifest_record = ManifestRecord {
        digest: manifest_digest.clone(),
        media_type: media_type.to_string(),
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
        image_name.as_ref(),
        RefConflictPolicy::KeepExisting,
    )?;
    // Public entry point: surface a ref conflict as `Err`. Callers
    // that need batch / report-style handling (e.g. legacy import)
    // drive `publish_artifact_atomic` directly with their own policy.
    if let Some(RefUpdate::Conflicted {
        existing_manifest_digest,
        incoming_manifest_digest,
    }) = &outcome.ref_update
    {
        if let Some(image_name) = image_name.as_ref() {
            anyhow::bail!(
                "Local registry ref conflict for {image_name}: existing manifest \
                 {existing_manifest_digest}, incoming manifest {incoming_manifest_digest}"
            );
        }
    }

    Ok(OciDirImport {
        manifest_digest,
        image_name,
        ref_update: outcome.ref_update,
    })
}

/// Single tar pass: write blob entries to [`FileBlobStore`], capture
/// `oci-layout` + `index.json` bytes for later parsing.
fn scan_archive<R: Read>(reader: R, blobs: &FileBlobStore, archive_path: &Path) -> Result<Scanned> {
    let mut archive = Archive::new(reader);
    let mut scanned = Scanned::default();
    for entry in archive
        .entries()
        .with_context(|| format!("Failed to read tar entries in {}", archive_path.display()))?
    {
        let mut entry = entry
            .with_context(|| format!("Failed to read tar entry in {}", archive_path.display()))?;
        let path = entry
            .path()
            .with_context(|| {
                format!(
                    "Failed to decode tar entry path in {}",
                    archive_path.display()
                )
            })?
            .into_owned();
        let path_str = path.to_string_lossy();

        // tar archives can carry directory entries; OCI Image Layout
        // archives don't strictly need them, but `tar -cf` adds them
        // when archiving a directory tree. Skip non-regular entries.
        let header = entry.header();
        if !matches!(header.entry_type(), tar::EntryType::Regular) {
            continue;
        }

        if path_str == "oci-layout" {
            let mut bytes = Vec::with_capacity(header.size().unwrap_or(0) as usize);
            entry.read_to_end(&mut bytes).with_context(|| {
                format!("Failed to read oci-layout from {}", archive_path.display())
            })?;
            scanned.oci_layout = Some(bytes);
            continue;
        }
        if path_str == "index.json" {
            let mut bytes = Vec::with_capacity(header.size().unwrap_or(0) as usize);
            entry.read_to_end(&mut bytes).with_context(|| {
                format!("Failed to read index.json from {}", archive_path.display())
            })?;
            scanned.index_json = Some(bytes);
            continue;
        }
        if let Some(digest) = blob_path_to_digest(&path_str) {
            let mut bytes = Vec::with_capacity(header.size().unwrap_or(0) as usize);
            entry.read_to_end(&mut bytes).with_context(|| {
                format!(
                    "Failed to read blob entry {path_str} from {}",
                    archive_path.display()
                )
            })?;
            let actual_digest = sha256_digest(&bytes);
            anyhow::ensure!(
                actual_digest == digest,
                "Blob digest mismatch in archive {}: entry path is {path_str}, sha256 is {actual_digest}",
                archive_path.display(),
            );
            blobs
                .put_bytes(&bytes)
                .with_context(|| format!("Failed to write blob {digest} to FileBlobStore"))?;
            continue;
        }
        // Forwards-compatible: ignore unknown entries (referrers,
        // signatures, …) rather than rejecting them at import.
    }
    Ok(scanned)
}

#[derive(Default)]
struct Scanned {
    oci_layout: Option<Vec<u8>>,
    index_json: Option<Vec<u8>>,
}

/// Convert a tar path like `blobs/sha256/<encoded>` into the canonical
/// `algorithm:encoded` digest form. Returns `None` if the path is not
/// a blob entry (so the caller can skip it).
fn blob_path_to_digest(path: &str) -> Option<String> {
    let rest = path.strip_prefix("blobs/")?;
    let (algorithm, encoded) = rest.split_once('/')?;
    if encoded.is_empty() || encoded.contains('/') {
        return None;
    }
    let candidate = format!("{algorithm}:{encoded}");
    // Reject paths that aren't a structurally-valid digest so we don't
    // silently treat e.g. a stray text file under `blobs/` as a blob.
    ValidatedDigest::parse(&candidate).ok()?;
    Some(candidate)
}

/// Validate that the archive carries the OMMX `artifactType`. Other
/// OCI artifacts are intentionally rejected — the v3 SDK is OMMX-
/// specific.
fn ensure_ommx_artifact_type(artifact_type: Option<&MediaType>) -> Result<()> {
    let artifact_type =
        artifact_type.context("OCI archive is not an OMMX artifact: artifactType is missing")?;
    anyhow::ensure!(
        artifact_type == &media_types::v1_artifact(),
        "OCI archive is not an OMMX artifact: {artifact_type}"
    );
    Ok(())
}

/// Build a [`BlobRecord`] for a manifest-referenced blob. The blob is
/// already on disk in [`FileBlobStore`] (written during the tar scan);
/// this is a metadata-only construction.
fn record_for_blob(
    blobs: &FileBlobStore,
    descriptor: &Descriptor,
    kind: &str,
    archive_path: &Path,
) -> Result<BlobRecord> {
    let digest = descriptor.digest().to_string();
    anyhow::ensure!(
        blobs.exists(&digest)?,
        "{kind} blob {digest} referenced by manifest is missing from {}",
        archive_path.display()
    );
    Ok(BlobRecord {
        digest: digest.clone(),
        size: descriptor.size(),
        media_type: Some(descriptor.media_type().to_string()),
        storage_uri: blobs
            .path_for_digest(&digest)?
            .to_string_lossy()
            .into_owned(),
        kind: kind.to_string(),
        last_verified_at: Some(now_rfc3339()),
    })
}

/// Build a [`BlobRecord`] for the manifest blob itself.
fn record_for_manifest_blob(blobs: &FileBlobStore, digest: &str, size: u64) -> Result<BlobRecord> {
    Ok(BlobRecord {
        digest: digest.to_string(),
        size,
        media_type: Some(OCI_IMAGE_MANIFEST_MEDIA_TYPE.to_string()),
        storage_uri: blobs
            .path_for_digest(digest)?
            .to_string_lossy()
            .into_owned(),
        kind: BLOB_KIND_MANIFEST.to_string(),
        last_verified_at: Some(now_rfc3339()),
    })
}

/// Extract the `org.opencontainers.image.ref.name` annotation from the
/// `index.json` manifest descriptor.
fn image_name_from_index_descriptor(desc: &Descriptor) -> Result<Option<ImageName>> {
    desc.annotations()
        .as_ref()
        .and_then(|annotations| annotations.get(OCI_IMAGE_REF_NAME_ANNOTATION))
        .map(|name| ImageName::parse(name).with_context(|| format!("Invalid image ref: {name}")))
        .transpose()
}
