//! Native `LocalArtifact::save` — SQLite + CAS → on-disk OCI archive.
//!
//! Streams the SQLite-resident artifact directly to an `OciArchiveBuilder`:
//! every layer / config blob is read from the BlobStore by digest and
//! appended via `ImageBuilder::add_blob`, then the manifest is finalised
//! through `ImageBuilder::build`. No intermediate on-disk OCI directory
//! is materialised.
//!
//! `OciArchiveBuilder` re-hashes each blob with sha256 to produce the
//! descriptor it embeds in the archive's index. We compare that fresh
//! digest against the digest the SQLite manifest claims so a corrupted
//! BlobStore surfaces here instead of producing a silently mis-tagged
//! archive.

use super::LocalArtifact;
use anyhow::Context;
use oci_spec::image::{Descriptor, Digest, ImageManifest};
use ocipkg::image::{ImageBuilder, OciArchiveBuilder};
use std::{path::Path, str::FromStr};

impl LocalArtifact {
    /// Pack this artifact into a `.ommx` OCI archive at `output`.
    ///
    /// Identity-preserving for the descriptors but **not byte-identical
    /// for the manifest blob**: `OciArchiveBuilder::build` re-serialises
    /// the parsed `ImageManifest`, which can produce a different byte
    /// representation (and hence a different manifest digest) when the
    /// original was serialised with non-canonical whitespace / field
    /// ordering. Matches the v2 `Artifact<OciDir>::save` round-trip.
    pub fn save(&self, output: &Path) -> crate::Result<()> {
        if output.exists() {
            crate::bail!("Output file already exists: {}", output.display());
        }
        let manifest_bytes = self.get_blob(self.manifest_digest())?;
        let manifest: ImageManifest = serde_json::from_slice(&manifest_bytes)
            .context("Failed to parse manifest from SQLite Local Registry")?;

        let mut builder = OciArchiveBuilder::new(output.to_path_buf(), self.image_name().clone())?;

        write_blob(&mut builder, self, manifest.config(), "config")?;
        for layer in manifest.layers() {
            write_blob(&mut builder, self, layer, "layer")?;
        }
        builder.build(manifest)?;
        Ok(())
    }
}

fn write_blob(
    builder: &mut OciArchiveBuilder,
    artifact: &LocalArtifact,
    descriptor: &Descriptor,
    kind: &str,
) -> crate::Result<()> {
    let expected = descriptor.digest().to_string();
    let bytes = artifact.get_blob(&expected)?;
    let expected_digest = Digest::from_str(&expected)
        .with_context(|| format!("Invalid {kind} digest in manifest: {expected}"))?;
    let (digest, size) = builder.add_blob(&bytes)?;
    crate::ensure!(
        digest == expected_digest,
        "{kind} digest mismatch on save: manifest claims {expected_digest}, blob bytes hash to {digest}",
    );
    crate::ensure!(
        size == descriptor.size(),
        "{kind} size mismatch on save: manifest claims {}, blob is {} bytes",
        descriptor.size(),
        size,
    );
    Ok(())
}
