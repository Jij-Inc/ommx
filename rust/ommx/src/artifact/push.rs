//! Native `LocalArtifact::push` — SQLite + CAS → remote OCI registry.
//!
//! The push streams the SQLite-resident artifact directly to the remote
//! registry: the empty config blob and every layer blob are read via
//! their descriptors only when missing from the destination, then pushed
//! through the `remote_transport::RemoteTransport` wrapper, then the
//! verbatim manifest bytes (as digest-addressed in the Local Registry) are
//! published with `application/vnd.oci.image.manifest.v1+json` as the
//! Content-Type. No intermediate on-disk OCI directory is materialised.
//!
//! `LocalManifest` is OCI Image Manifest only — Artifact Manifest is
//! rejected at parse time — so blob enumeration is uniform: `config`
//! followed by each entry in `layers[]`.

use super::{
    remote_transport::{bounded_map, RemoteTransport},
    LocalArtifact, LocalManifest,
};
use anyhow::Context;
use oci_client::RegistryOperation;
use oci_spec::image::Descriptor;
use std::collections::HashMap;

const BLOB_CHECK_CONCURRENCY: usize = 16;
const BLOB_UPLOAD_CONCURRENCY: usize = 4;

impl LocalArtifact<'_> {
    /// Push this artifact to its OCI registry. Credentials are
    /// resolved by `super::remote_transport`'s three-tier chain:
    /// `OMMX_BASIC_AUTH_*` env vars (explicit override) →
    /// `~/.docker/config.json` plus credential helpers
    /// (`docker-credential-gcloud`, …) → anonymous. A workstation
    /// `docker login` is sufficient; OMMX does not maintain its own
    /// credential store.
    ///
    /// Pushes blobs first, manifest last, so a partial failure leaves
    /// the registry without a tag pointing at incomplete data. Blobs
    /// already present at the destination are skipped after a remote
    /// existence check; missing blobs are read from the Local Registry
    /// and uploaded. The blob phase authenticates for pull-scoped
    /// existence checks first, then for push-scoped uploads and
    /// manifest publishing.
    pub fn push(&self) -> crate::Result<()> {
        let manifest = self.get_manifest()?.clone();
        let blob_descriptors = collect_blob_descriptors(&manifest);
        let unique_blob_descriptors = deduplicate_blob_descriptors(&blob_descriptors)?;
        let deduplicated = blob_descriptors.len() - unique_blob_descriptors.len();

        let transport = RemoteTransport::new(self.image_name())?;
        transport.auth_for(self.image_name(), RegistryOperation::Pull)?;
        let missing_blob_descriptors = transport.block_on(async {
            let checked = bounded_map(
                unique_blob_descriptors,
                BLOB_CHECK_CONCURRENCY,
                |descriptor| {
                    let transport = &transport;
                    async move {
                        let digest = descriptor.digest().to_string();
                        let exists = transport
                            .blob_exists_async(self.image_name(), &digest)
                            .await
                            .with_context(|| format!("Failed while checking blob {digest}"))?;
                        Ok::<_, crate::Error>((descriptor, exists))
                    }
                },
            )
            .await?;
            Ok::<_, crate::Error>(missing_descriptors(checked))
        })?;
        let checked = blob_descriptors.len() - deduplicated;
        let skipped = checked - missing_blob_descriptors.len();
        tracing::info!(
            checked,
            skipped,
            deduplicated,
            missing = missing_blob_descriptors.len(),
            "Completed remote blob existence checks"
        );

        transport.auth(self.image_name())?;
        let transferred = missing_blob_descriptors.len();
        transport.block_on(async {
            bounded_map(
                missing_blob_descriptors,
                BLOB_UPLOAD_CONCURRENCY,
                |descriptor| {
                    let transport = &transport;
                    async move {
                        let digest = descriptor.digest().to_string();
                        // Blob loading happens only when this bounded future is polled, so at
                        // most BLOB_UPLOAD_CONCURRENCY owned buffers are resident here.
                        let bytes = self.get_blob_by_descriptor(descriptor)?;
                        tracing::debug!(size = bytes.len(), %digest, "Pushing blob");
                        transport
                            .push_blob_async(self.image_name(), &digest, bytes)
                            .await
                            .with_context(|| format!("Failed while uploading blob {digest}"))
                    }
                },
            )
            .await
        })?;
        tracing::info!(transferred, "Completed remote blob uploads");

        let manifest_bytes = self.read_blob_by_digest(self.manifest_digest())?;
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

fn deduplicate_blob_descriptors(descriptors: &[Descriptor]) -> crate::Result<Vec<&Descriptor>> {
    let mut seen = HashMap::with_capacity(descriptors.len());
    let mut unique = Vec::with_capacity(descriptors.len());
    for descriptor in descriptors {
        let digest = descriptor.digest().to_string();
        if let Some(size) = seen.get(&digest) {
            anyhow::ensure!(
                *size == descriptor.size(),
                "Conflicting sizes for blob {digest}: {size} and {}",
                descriptor.size()
            );
        } else {
            seen.insert(digest, descriptor.size());
            unique.push(descriptor);
        }
    }
    Ok(unique)
}

fn missing_descriptors(checked: Vec<(&Descriptor, bool)>) -> Vec<&Descriptor> {
    checked
        .into_iter()
        .filter_map(|(descriptor, exists)| {
            if exists {
                tracing::debug!(digest = %descriptor.digest(), "Skipping blob already present in remote");
                None
            } else {
                Some(descriptor)
            }
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;
    use oci_spec::image::{DescriptorBuilder, Digest, MediaType};
    use std::str::FromStr;

    fn descriptor_for(bytes: &[u8]) -> Descriptor {
        DescriptorBuilder::default()
            .media_type(MediaType::Other("application/octet-stream".to_string()))
            .digest(Digest::from_str(&crate::artifact::sha256_digest(bytes)).unwrap())
            .size(bytes.len() as u64)
            .build()
            .unwrap()
    }

    #[test]
    fn duplicate_descriptors_are_deduplicated_by_digest() {
        let first = descriptor_for(b"same blob");
        let duplicate = first.clone();
        let other = descriptor_for(b"other blob");
        let descriptors = vec![first, duplicate, other];

        let unique = deduplicate_blob_descriptors(&descriptors).unwrap();

        assert_eq!(unique.len(), 2);
        assert_eq!(unique[0].digest(), descriptors[0].digest());
        assert_eq!(unique[1].digest(), descriptors[2].digest());
    }

    #[test]
    fn already_present_descriptors_are_not_uploaded() {
        let present = descriptor_for(b"present");
        let missing = descriptor_for(b"missing");

        let selected = missing_descriptors(vec![(&present, true), (&missing, false)]);

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].digest(), missing.digest());
    }
}
