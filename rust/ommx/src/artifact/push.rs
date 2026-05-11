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

#[cfg(test)]
mod tests {
    use crate::artifact::{
        local_registry::{LocalRegistry, RefConflictPolicy},
        media_types, LocalArtifactBuilder,
    };
    use anyhow::Result;
    use oci_spec::image::MediaType;
    use ocipkg::ImageName;
    use std::collections::HashMap;
    use std::sync::Arc;

    /// Pushes an OCI Artifact Manifest produced by the v3 SQLite Local
    /// Registry to a `registry:2`-style local registry on
    /// `localhost:5000`. Ignored by default; the `with-registry` CI job
    /// re-runs the test suite with `--include-ignored` after launching
    /// the registry service container.
    ///
    /// The test only asserts that the entire push path (auth → blob
    /// upload → manifest upload with `application/vnd.oci.artifact.manifest.v1+json`
    /// Content-Type) completes against a real registry. Registry-state
    /// verification (`curl /v2/<repo>/tags/list`) is done in the CI step
    /// rather than in Rust so the test stays self-contained.
    #[test]
    #[ignore]
    fn push_local_artifact_to_localhost_registry() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let registry = Arc::new(LocalRegistry::open(dir.path())?);
        let image_name = ImageName::parse("localhost:5000/ommx-test/native-push:tag1")?;

        let mut builder = LocalArtifactBuilder::new(image_name.clone());
        builder.add_layer_bytes(
            MediaType::Other(media_types::V1_INSTANCE_MEDIA_TYPE.to_string()),
            b"native-push".to_vec(),
            HashMap::from([(
                "org.ommx.v1.instance.title".to_string(),
                "native-push".to_string(),
            )]),
        )?;
        let artifact = builder.build_in_registry(registry.clone(), RefConflictPolicy::Replace)?;

        artifact.push()
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
