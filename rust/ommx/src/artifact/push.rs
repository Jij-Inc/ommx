//! Native `LocalArtifact::push` — SQLite + CAS → remote OCI registry.
//!
//! The push streams the SQLite-resident artifact directly to the remote
//! registry: every layer / blob is read from the BlobStore by digest and
//! pushed through the [`super::remote_transport::RemoteTransport`]
//! wrapper, then the verbatim manifest bytes (as digest-addressed in the
//! BlobStore) are published with the manifest's recorded media type. No
//! intermediate on-disk OCI directory is materialised.
//!
//! Image Manifest and Artifact Manifest are handled uniformly because
//! the manifest is treated as opaque bytes: the only `LocalManifest`
//! dispatch needed is "collect descriptors to push" — for an Image
//! Manifest that is `config + layers`, for an Artifact Manifest it is
//! `optional config + blobs`. See `super::manifest::LocalManifest`.

use super::{remote_transport::RemoteTransport, LocalArtifact, LocalManifest};
use oci_spec::image::Descriptor;

impl LocalArtifact {
    /// Push this artifact to its OCI registry. The credentials are read
    /// from `OMMX_BASIC_AUTH_DOMAIN` / `OMMX_BASIC_AUTH_USERNAME` /
    /// `OMMX_BASIC_AUTH_PASSWORD` env vars; absence means anonymous.
    ///
    /// Pushes blobs first, manifest last, so a partial failure leaves
    /// the registry without a tag pointing at incomplete data. Blobs
    /// already present at the destination are still re-uploaded — the
    /// OCI distribution protocol's mount/cross-repo-blob optimisation is
    /// a Step B+ refinement.
    pub fn push(&self) -> crate::Result<()> {
        let manifest = self.get_manifest()?.clone();
        let blob_descriptors = collect_blob_descriptors(&manifest);

        let transport = RemoteTransport::new(self.image_name())?;
        transport.auth(self.image_name())?;

        for descriptor in &blob_descriptors {
            let digest = descriptor.digest().to_string();
            let bytes = self.get_blob(&digest)?;
            tracing::debug!(
                size = bytes.len(),
                "Pushing blob {digest} of {}",
                self.image_name()
            );
            transport.push_blob(self.image_name(), &digest, &bytes)?;
        }

        let manifest_bytes = self.get_blob(self.manifest_digest())?;
        let content_type = manifest.media_type();
        tracing::info!(
            "Publishing manifest {} ({}, {} bytes) to {}",
            self.manifest_digest(),
            content_type,
            manifest_bytes.len(),
            self.image_name(),
        );
        transport.push_manifest_bytes(self.image_name(), &manifest_bytes, content_type)?;
        Ok(())
    }
}

/// Enumerate every blob a manifest references, in push order: dependent
/// blobs (`config`, `layers` / `blobs`) before the manifest itself.
fn collect_blob_descriptors(manifest: &LocalManifest) -> Vec<Descriptor> {
    match manifest {
        LocalManifest::Image(m) => {
            let mut out = Vec::with_capacity(1 + m.layers().len());
            out.push(m.config().clone());
            out.extend(m.layers().iter().cloned());
            out
        }
        LocalManifest::Artifact(m) => m.blobs().to_vec(),
    }
}
