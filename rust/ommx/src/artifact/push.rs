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

use super::{remote_transport::RemoteTransport, LocalArtifact, LocalManifest};
use oci_spec::image::Descriptor;

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
    /// and uploaded.
    pub fn push(&self) -> crate::Result<()> {
        let manifest = self.get_manifest()?.clone();
        let blob_descriptors = collect_blob_descriptors(&manifest);

        let transport = RemoteTransport::new(self.image_name())?;
        transport.auth(self.image_name())?;

        for descriptor in &blob_descriptors {
            push_descriptor_blob(
                self.image_name(),
                descriptor,
                |descriptor| self.get_blob_by_descriptor(descriptor),
                |digest| transport.blob_exists(self.image_name(), digest),
                |digest, bytes| transport.push_blob(self.image_name(), digest, bytes),
            )?;
        }

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

/// Enumerate every blob a manifest references, in push order: dependent
/// blobs (`config`, then `layers`) before the manifest itself.
fn collect_blob_descriptors(manifest: &LocalManifest) -> Vec<Descriptor> {
    let layers = manifest.layers();
    let mut out = Vec::with_capacity(1 + layers.len());
    out.push(manifest.config());
    out.extend(layers);
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlobPushAction {
    SkippedExisting,
    Uploaded,
}

fn push_descriptor_blob(
    image_name: &crate::artifact::ImageRef,
    descriptor: &Descriptor,
    read_blob: impl FnOnce(&Descriptor) -> crate::Result<Vec<u8>>,
    remote_blob_exists: impl FnOnce(&str) -> crate::Result<bool>,
    push_blob: impl FnOnce(&str, Vec<u8>) -> crate::Result<()>,
) -> crate::Result<BlobPushAction> {
    let digest = descriptor.digest().to_string();
    if remote_blob_exists(&digest)? {
        tracing::debug!("Skipping blob {digest} of {image_name}; already present in remote");
        return Ok(BlobPushAction::SkippedExisting);
    }

    let bytes = read_blob(descriptor)?;
    tracing::debug!(size = bytes.len(), "Pushing blob {digest} of {image_name}");
    // `bytes` is moved into `push_blob`, which moves it into
    // `oci_client::Client::push_blob`, which takes `Vec<u8>` by
    // value. Avoid `to_vec()`-ing a buffer that is already owned
    // (blobs can be tens of MB).
    push_blob(&digest, bytes)?;
    Ok(BlobPushAction::Uploaded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{anyhow, Context};
    use oci_spec::image::{DescriptorBuilder, Digest, MediaType};
    use std::{
        cell::{Cell, RefCell},
        str::FromStr,
    };

    fn image_name() -> crate::artifact::ImageRef {
        crate::artifact::ImageRef::parse("ghcr.io/jij-inc/ommx/demo:push-test").unwrap()
    }

    fn descriptor_for(bytes: &[u8]) -> Descriptor {
        DescriptorBuilder::default()
            .media_type(MediaType::Other("application/octet-stream".to_string()))
            .digest(Digest::from_str(&crate::artifact::sha256_digest(bytes)).unwrap())
            .size(bytes.len() as u64)
            .build()
            .unwrap()
    }

    #[test]
    fn existing_remote_blob_skips_local_read_and_upload() -> crate::Result<()> {
        let image_name = image_name();
        let descriptor = descriptor_for(b"already remote");
        let checked_digest = RefCell::new(None);
        let read_count = Cell::new(0);
        let push_count = Cell::new(0);

        let action = push_descriptor_blob(
            &image_name,
            &descriptor,
            |_| {
                read_count.set(read_count.get() + 1);
                Ok(Vec::new())
            },
            |digest| {
                checked_digest.replace(Some(digest.to_string()));
                Ok(true)
            },
            |_, _| {
                push_count.set(push_count.get() + 1);
                Ok(())
            },
        )?;

        assert_eq!(action, BlobPushAction::SkippedExisting);
        let expected_digest = descriptor.digest().to_string();
        assert_eq!(
            checked_digest.into_inner().as_deref(),
            Some(expected_digest.as_str())
        );
        assert_eq!(read_count.get(), 0);
        assert_eq!(push_count.get(), 0);
        Ok(())
    }

    #[test]
    fn missing_remote_blob_reads_and_uploads_once() -> crate::Result<()> {
        let image_name = image_name();
        let bytes = b"needs upload".to_vec();
        let descriptor = descriptor_for(&bytes);
        let read_count = Cell::new(0);
        let pushed = RefCell::new(Vec::new());

        let action = push_descriptor_blob(
            &image_name,
            &descriptor,
            |_| {
                read_count.set(read_count.get() + 1);
                Ok(bytes.clone())
            },
            |_| Ok(false),
            |digest, bytes| {
                pushed.borrow_mut().push((digest.to_string(), bytes));
                Ok(())
            },
        )?;

        assert_eq!(action, BlobPushAction::Uploaded);
        assert_eq!(read_count.get(), 1);
        let pushed = pushed.into_inner();
        assert_eq!(pushed.len(), 1);
        assert_eq!(pushed[0].0, descriptor.digest().to_string());
        assert_eq!(pushed[0].1, b"needs upload");
        Ok(())
    }

    #[test]
    fn remote_check_error_stops_before_read_or_upload() {
        let image_name = image_name();
        let descriptor = descriptor_for(b"unreachable remote");
        let read_count = Cell::new(0);
        let push_count = Cell::new(0);

        let err = push_descriptor_blob(
            &image_name,
            &descriptor,
            |_| {
                read_count.set(read_count.get() + 1);
                Ok(Vec::new())
            },
            |_| Err(anyhow!("registry HEAD failed")),
            |_, _| {
                push_count.set(push_count.get() + 1);
                Ok(())
            },
        )
        .context("push decision should fail")
        .unwrap_err();

        assert!(err.to_string().contains("push decision should fail"));
        assert_eq!(read_count.get(), 0);
        assert_eq!(push_count.get(), 0);
    }
}
