//! Native `LocalArtifact::save` — SQLite + CAS → on-disk OCI archive.
//!
//! Step F (§12.4) replaced the previous `OciArchiveBuilder`-based
//! writer (which re-serialised the parsed `ImageManifest` and so could
//! shift the manifest digest under non-canonical input) with a direct
//! tar writer over the [`tar`] crate. The resulting `.ommx` file is
//! the standard "tar of OCI Image Layout": an `oci-layout` marker, a
//! one-entry `index.json`, and `blobs/sha256/<digest>` entries for
//! the manifest + config + every layer.
//!
//! v2 round-trip preserved: the manifest bytes the SQLite Local
//! Registry already stores are appended verbatim, so the manifest
//! digest is byte-identical to the digest the artifact was published
//! under. `OciArchive` readers (including the v3 native
//! [`super::local_registry::import_oci_archive`]) see the same digest
//! they would see for the source artifact.
//!
//! Memory shape: each blob is read into a `Vec<u8>` and streamed
//! through the tar writer; the writer itself is `BufWriter<File>`. A
//! 200 MB layer therefore peaks at ~200 MB resident memory during the
//! save, which is the same shape the v2 archive build had. A future
//! refinement that streams blobs out of [`super::FileBlobStore`] via
//! `std::io::copy` (the `FileBlobStore` already keeps each blob in its
//! own file) would replace the `Vec<u8>` allocation with a fixed
//! 64 KB copy buffer.

use super::{
    local_registry::{ValidatedDigest, OCI_IMAGE_REF_NAME_ANNOTATION},
    LocalArtifact,
};
use anyhow::Context;
use oci_spec::image::{
    Descriptor, DescriptorBuilder, Digest, ImageIndexBuilder, ImageManifest, MediaType,
    OciLayoutBuilder,
};
use std::{
    collections::HashMap,
    fs::OpenOptions,
    io::{BufWriter, Cursor, Write},
    path::Path,
    str::FromStr,
};
use tar::{EntryType, Header};

/// OCI Image Layout version this archive declares. The OCI spec
/// pins this string at `1.0.0`; importers reject anything else.
const OCI_LAYOUT_VERSION: &str = "1.0.0";

impl LocalArtifact {
    /// Pack this artifact into a `.ommx` OCI archive at `output`.
    ///
    /// Identity-preserving: the manifest bytes the SQLite Local
    /// Registry holds are written verbatim, so the manifest digest
    /// is byte-identical across the source SQLite registry and the
    /// produced archive. Importing the archive back into a fresh
    /// registry round-trips to the same digest.
    pub fn save(&self, output: &Path) -> crate::Result<()> {
        let manifest_digest = self.manifest_digest().to_string();
        let manifest_bytes = self.get_blob(&manifest_digest)?;
        let manifest: ImageManifest = serde_json::from_slice(&manifest_bytes)
            .context("Failed to parse manifest from SQLite Local Registry")?;

        // `create_new(true)` closes the TOCTOU window between an `exists()`
        // probe and the open: a concurrent process that races us cannot
        // have its file silently truncated. The kernel's atomic
        // O_CREAT|O_EXCL is the only safe surface here.
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output)
            .with_context(|| {
                format!(
                    "Failed to create archive at {} (file already exists or path is invalid)",
                    output.display()
                )
            })?;
        let mut tar = tar::Builder::new(BufWriter::new(file));
        // Deterministic mode pins uid/gid/mtime to known values so a
        // bit-for-bit `save` against the same registry produces the
        // same archive bytes — useful for caching, content-addressed
        // build outputs, and CI reproducibility checks.
        tar.mode(tar::HeaderMode::Deterministic);

        // 1. `oci-layout` marker.
        let oci_layout = OciLayoutBuilder::default()
            .image_layout_version(OCI_LAYOUT_VERSION.to_string())
            .build()
            .context("Failed to build oci-layout JSON")?;
        let layout_bytes =
            serde_json::to_vec(&oci_layout).context("Failed to serialise oci-layout JSON")?;
        append_tar_file(&mut tar, "oci-layout", &layout_bytes)?;

        // 2. Blobs — manifest first (so a `tar tvf` listing reads
        //    naturally from "what is the artifact" to "what does it
        //    contain"), then config, then layers. Order is not
        //    semantically significant — OCI Image Layout readers index
        //    by `blobs/<algorithm>/<encoded>` regardless of tar order.
        append_blob_entry(&mut tar, &manifest_digest, &manifest_bytes, "manifest")?;
        let config = manifest.config();
        let config_bytes = self.get_blob(config.digest().as_ref())?;
        verify_blob(config, config_bytes.len(), "config")?;
        append_blob_entry(&mut tar, config.digest().as_ref(), &config_bytes, "config")?;
        for layer in manifest.layers() {
            let layer_bytes = self.get_blob(layer.digest().as_ref())?;
            verify_blob(layer, layer_bytes.len(), "layer")?;
            append_blob_entry(&mut tar, layer.digest().as_ref(), &layer_bytes, "layer")?;
        }

        // 3. `index.json` — single-entry ImageIndex pointing at the
        //    manifest, annotated with the image ref so the archive can
        //    be imported back under the same name without a side
        //    channel.
        let manifest_digest_parsed = Digest::from_str(&manifest_digest)
            .with_context(|| format!("Invalid manifest digest: {manifest_digest}"))?;
        let manifest_descriptor = DescriptorBuilder::default()
            .media_type(MediaType::ImageManifest)
            .digest(manifest_digest_parsed)
            .size(manifest_bytes.len() as u64)
            .annotations({
                let mut map = HashMap::new();
                map.insert(
                    OCI_IMAGE_REF_NAME_ANNOTATION.to_string(),
                    self.image_name().to_string(),
                );
                map
            })
            .build()
            .context("Failed to build manifest descriptor for archive index.json")?;
        let image_index = ImageIndexBuilder::default()
            .schema_version(2u32)
            .media_type(MediaType::ImageIndex)
            .manifests(vec![manifest_descriptor])
            .build()
            .context("Failed to build OCI Image Index for archive index.json")?;
        let index_bytes = serde_json::to_vec(&image_index)
            .context("Failed to serialise OCI Image Index for archive index.json")?;
        append_tar_file(&mut tar, "index.json", &index_bytes)?;

        let mut writer = tar.into_inner().context("Failed to finalise tar archive")?;
        writer
            .flush()
            .with_context(|| format!("Failed to flush archive writer to {}", output.display()))?;
        Ok(())
    }
}

/// Append a regular file at `path` with the given bytes. Path is
/// relative to the archive root (no leading `/`).
fn append_tar_file<W: Write>(
    tar: &mut tar::Builder<W>,
    path: &str,
    bytes: &[u8],
) -> crate::Result<()> {
    let mut header = Header::new_ustar();
    header.set_size(bytes.len() as u64);
    header.set_mode(0o644);
    header.set_mtime(0);
    header.set_entry_type(EntryType::Regular);
    header
        .set_path(path)
        .with_context(|| format!("Tar entry path {path} is not representable in USTAR"))?;
    header.set_cksum();
    tar.append(&header, Cursor::new(bytes))
        .with_context(|| format!("Failed to append tar entry {path}"))
}

/// Append a CAS blob under `blobs/<algorithm>/<encoded>`. `digest` is
/// the full `algorithm:encoded` form the manifest stores.
fn append_blob_entry<W: Write>(
    tar: &mut tar::Builder<W>,
    digest: &str,
    bytes: &[u8],
    kind: &str,
) -> crate::Result<()> {
    let parsed = ValidatedDigest::parse(digest).with_context(|| {
        format!("Invalid {kind} digest while writing archive blob entry: {digest}")
    })?;
    let path = format!("blobs/{}/{}", parsed.algorithm(), parsed.encoded());
    append_tar_file(tar, &path, bytes)
}

/// Cross-check a CAS blob's recorded size against what the manifest
/// descriptor claims so a corrupted BlobStore surfaces here instead of
/// producing a silently mis-tagged archive.
fn verify_blob(descriptor: &Descriptor, actual_size: usize, kind: &str) -> crate::Result<()> {
    crate::ensure!(
        actual_size as u64 == descriptor.size(),
        "{kind} size mismatch on save: manifest claims {}, blob is {} bytes",
        descriptor.size(),
        actual_size,
    );
    Ok(())
}
