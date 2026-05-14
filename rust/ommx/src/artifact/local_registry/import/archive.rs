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
//! write the manifest descriptor under the
//! `org.opencontainers.image.ref.name` annotated ref. A crash between
//! blob writes and ref publish leaves orphan CAS bytes recoverable by
//! GC; the SQLite index never stores a manifest / layer cache.

use super::super::{
    sha256_digest, FileBlobStore, LocalRegistry, RefConflictPolicy, RefUpdate, ValidatedDigest,
    OCI_IMAGE_REF_NAME_ANNOTATION,
};
use super::oci_dir::OciDirImport;
use crate::artifact::{media_types, ImageRef};
use anyhow::{Context, Result};
use oci_spec::image::{Descriptor, Digest, ImageIndex, ImageManifest, MediaType, OciLayout};
use std::{
    fs::File,
    io::{BufReader, Read},
    path::Path,
    sync::Arc,
};
use tar::Archive;

/// Read-only view of an OCI archive's identifying metadata: the
/// parsed manifest, its digest, and the image ref name annotated on
/// the index descriptor (when present). Produced by
/// [`inspect_archive`] without touching the SQLite Local Registry.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ArchiveInspectView {
    pub image_name: Option<ImageRef>,
    pub manifest: ImageManifest,
    pub manifest_digest: Digest,
}

/// Inspect a `.ommx` archive without importing it into the SQLite
/// Local Registry. Streams the tar twice: once for `index.json` (to
/// locate the manifest descriptor + ref name annotation) and once for
/// the manifest blob (verified by sha256). No layer / config blobs
/// are read — only the manifest, which is small enough that the
/// per-build allocation is negligible. Used by CLI
/// `ommx inspect <archive>` and Python `Artifact.inspect_archive`.
pub fn inspect_archive(path: &Path) -> Result<ArchiveInspectView> {
    let (manifest_digest, image_name) = read_archive_index(path)?;
    let manifest_bytes = read_archive_blob(path, &manifest_digest)?;
    let manifest: ImageManifest = serde_json::from_slice(&manifest_bytes).with_context(|| {
        format!(
            "Failed to parse OCI image manifest blob {manifest_digest} in {}",
            path.display()
        )
    })?;
    Ok(ArchiveInspectView {
        image_name,
        manifest,
        manifest_digest,
    })
}

/// First-pass helper for [`inspect_archive`]: stream the tar to find
/// `index.json` and return `(manifest_digest, ref_name)`.
fn read_archive_index(path: &Path) -> Result<(Digest, Option<ImageRef>)> {
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
        // `tar -cf foo.tar -C dir .` writes entries with a leading
        // `./`; both shapes describe the same OCI Image Layout.
        let raw = entry_path.to_string_lossy();
        let path_str = raw.strip_prefix("./").unwrap_or(&raw);
        if path_str != "index.json" {
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
        let descriptor = image_index.manifests().first().unwrap();
        let image_name = image_name_from_index_descriptor(descriptor)?;
        return Ok((descriptor.digest().clone(), image_name));
    }
    anyhow::bail!("Missing index.json in {}", path.display())
}

/// Second-pass helper for [`inspect_archive`]: stream the tar a
/// second time to read the blob at `digest` from
/// `blobs/<algorithm>/<encoded>`. Manifests are first in the v3
/// native writer's output so the scan terminates quickly there.
fn read_archive_blob(path: &Path, digest: &Digest) -> Result<Vec<u8>> {
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
        let raw = entry_path.to_string_lossy();
        let path_str = raw.strip_prefix("./").unwrap_or(&raw);
        if !matches!(entry.header().entry_type(), tar::EntryType::Regular) {
            continue;
        }
        if let Some(entry_digest) = blob_path_to_digest(path_str) {
            if entry_digest == digest.as_ref() {
                let mut bytes = Vec::with_capacity(entry.header().size().unwrap_or(0) as usize);
                entry.read_to_end(&mut bytes).with_context(|| {
                    format!("Failed to read blob {digest} from {}", path.display())
                })?;
                anyhow::ensure!(
                    sha256_digest(&bytes) == digest.as_ref(),
                    "Blob {digest} in {} fails sha256 check",
                    path.display()
                );
                return Ok(bytes);
            }
        }
    }
    anyhow::bail!(
        "Blob {digest} declared by index.json is missing from archive {}",
        path.display()
    );
}

/// Import a `.ommx` OCI archive on disk into the v3 SQLite Local Registry.
///
/// Streams the archive entries once: writes every `blobs/sha256/<digest>`
/// blob straight into [`FileBlobStore`] (which re-derives sha256 and
/// asserts the recomputed digest matches the tar path), captures
/// `oci-layout` + `index.json` into memory for the post-pass parse,
/// and finally emits a single SQLite transaction that publishes the
/// manifest + ref.
///
/// **Unnamed archives are accepted**: a `.ommx` whose `index.json`
/// descriptor lacks the `org.opencontainers.image.ref.name`
/// annotation (a shape v2-era OMMX SDKs produced in real workflows)
/// is imported under a freshly-synthesized anonymous ref name of the
/// form `<registry-id8>.ommx.local/anonymous:<timestamp>-<nonce>` —
/// the same shape `LocalArtifactBuilder::new_anonymous` produces.
/// The returned [`OciDirImport`]'s `image_name` is then `Some(...)`
/// with the synthesized name, so callers always have a way to address
/// the imported artifact. Each `import_oci_archive` call on the same
/// unnamed archive synthesizes a fresh name (the nonce differs), so
/// repeated imports accumulate distinct refs pointing at the same
/// manifest digest (CAS-deduped). Use `ommx artifact prune-anonymous`
/// to clean accumulated synthesized refs.
///
/// Returns the [`OciDirImport`] outcome reported by the underlying
/// publish (`Inserted` on first call for this image, `Unchanged` for
/// an idempotent re-import of the same digest under the same ref, or
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
    let manifest_digest = index_descriptor.digest().clone();
    // v2-era OMMX SDKs produced `.ommx` files whose `index.json`
    // descriptor lacks the `org.opencontainers.image.ref.name`
    // annotation (the v3 SDK always sets it). Rather than refuse to
    // import those — which would strand real v2 user workflows — we
    // synthesize an anonymous ref name here so the SQLite Local
    // Registry has a key to address the imported artifact under.
    // The synthesized name follows the same shape
    // `LocalArtifactBuilder::new_anonymous` produces, so
    // `ommx artifact prune-anonymous` cleans them by the same
    // structural match.
    let image_name = match image_name_from_index_descriptor(index_descriptor)? {
        Some(name) => name,
        None => {
            let registry_id = registry.index().registry_id()?;
            let synthesized = crate::artifact::anonymous_artifact_image_name(&registry_id)?;
            tracing::info!(
                "OCI archive at {} has no `org.opencontainers.image.ref.name` \
                 annotation; importing under synthesized anonymous name {synthesized}",
                path.display(),
            );
            synthesized
        }
    };

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
    match index_descriptor.media_type() {
        MediaType::ImageManifest => {}
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

    // All referenced blobs are already in `FileBlobStore` (written
    // during the tar scan). Verify the manifest is self-contained
    // before publishing the ref descriptor.
    ensure_blob_exists(registry.blobs(), manifest.config(), path)?;
    for layer in manifest.layers() {
        ensure_blob_exists(registry.blobs(), layer, path)?;
    }

    let ref_update = registry.index().put_image_ref_with_policy(
        &image_name,
        index_descriptor,
        RefConflictPolicy::KeepExisting,
    )?;
    // Public entry point: surface a ref conflict as `Err`. Callers
    // that need batch / report-style handling (e.g. legacy import)
    // use the directory import path, which can return conflicts.
    if let RefUpdate::Conflicted {
        existing_manifest_digest,
        incoming_manifest_digest,
    } = &ref_update
    {
        anyhow::bail!(
            "Local registry ref conflict for {image_name}: existing manifest \
             {existing_manifest_digest}, incoming manifest {incoming_manifest_digest}"
        );
    }

    Ok(OciDirImport {
        manifest_digest,
        image_name: Some(image_name),
        ref_update: Some(ref_update),
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
        let raw_path_str = path.to_string_lossy();
        // `tar -cf foo.tar -C dir .` writes every member with a
        // leading `./`; both shapes describe the same OCI Image
        // Layout, so normalise before matching against the well-known
        // `oci-layout` / `index.json` / `blobs/...` paths.
        let path_str = raw_path_str.strip_prefix("./").unwrap_or(&raw_path_str);

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
        if let Some(digest) = blob_path_to_digest(path_str) {
            let mut bytes = Vec::with_capacity(header.size().unwrap_or(0) as usize);
            entry.read_to_end(&mut bytes).with_context(|| {
                format!(
                    "Failed to read blob entry {path_str} from {}",
                    archive_path.display()
                )
            })?;
            // `put_bytes` hashes once internally and returns the
            // digest of what it stored; comparing against the
            // expected `digest` (derived from the entry path) costs
            // one string compare instead of a second SHA-256 pass.
            let actual_digest = blobs
                .put_bytes(&bytes)
                .with_context(|| format!("Failed to write blob {digest} to FileBlobStore"))?;
            anyhow::ensure!(
                actual_digest.as_ref() == digest,
                "Blob digest mismatch in archive {}: entry path is {path_str}, sha256 is {}",
                archive_path.display(),
                actual_digest,
            );
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

/// Verify a manifest-referenced blob is present in [`FileBlobStore`].
/// The blob was written during the tar scan; the SQLite index only
/// needs the manifest descriptor.
fn ensure_blob_exists(
    blobs: &FileBlobStore,
    descriptor: &Descriptor,
    archive_path: &Path,
) -> Result<()> {
    let digest = descriptor.digest();
    anyhow::ensure!(
        blobs.exists(digest)?,
        "Blob {digest} referenced by manifest is missing from {}",
        archive_path.display()
    );
    Ok(())
}

/// Extract the `org.opencontainers.image.ref.name` annotation from the
/// `index.json` manifest descriptor.
fn image_name_from_index_descriptor(desc: &Descriptor) -> Result<Option<ImageRef>> {
    desc.annotations()
        .as_ref()
        .and_then(|annotations| annotations.get(OCI_IMAGE_REF_NAME_ANNOTATION))
        .map(|name| ImageRef::parse(name).with_context(|| format!("Invalid image ref: {name}")))
        .transpose()
}
