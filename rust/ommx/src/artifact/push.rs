//! Native `LocalArtifact::push` — SQLite + CAS → remote OCI registry.
//!
//! The push streams the SQLite-resident artifact directly to the remote
//! registry: the empty config blob and every layer blob are read from
//! the BlobStore by digest and pushed through the
//! [`super::remote_transport::RemoteTransport`] wrapper, then the
//! verbatim manifest bytes (as digest-addressed in the BlobStore) are
//! published with `application/vnd.oci.image.manifest.v1+json` as the
//! Content-Type. No intermediate on-disk OCI directory is materialised.
//!
//! `LocalManifest` is OCI Image Manifest only — Artifact Manifest is
//! rejected at parse time — so blob enumeration is uniform: `config`
//! followed by each entry in `layers[]`.

use super::{remote_transport::RemoteTransport, LocalArtifact, LocalManifest};
use oci_spec::image::Descriptor;

impl LocalArtifact {
    /// Push this artifact to its OCI registry. Credentials are
    /// resolved by [`super::remote_transport`]'s three-tier chain:
    /// `OMMX_BASIC_AUTH_*` env vars (explicit override) →
    /// `~/.docker/config.json` plus credential helpers
    /// (`docker-credential-gcloud`, …) → anonymous. A workstation
    /// `docker login` is sufficient; OMMX does not maintain its own
    /// credential store.
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
            // `bytes` is moved into `push_blob`, which moves it into
            // `oci_client::Client::push_blob`, which takes `Vec<u8>`
            // by value. Avoid `to_vec()`-ing a buffer that is
            // already owned (blobs can be tens of MB).
            transport.push_blob(self.image_name(), &digest, bytes)?;
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
        transport.push_manifest_bytes(self.image_name(), manifest_bytes, content_type)?;
        Ok(())
    }
}

/// Enumerate every blob a manifest references, in push order: dependent
/// blobs (`config`, then `layers`) before the manifest itself.
fn collect_blob_descriptors(manifest: &LocalManifest) -> Vec<Descriptor> {
    let layers = manifest.layers();
    let mut out = Vec::with_capacity(1 + layers.len());
    out.push(manifest.config());
    out.extend(layers);
    out
}
