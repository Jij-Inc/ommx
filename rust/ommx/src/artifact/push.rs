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
    manifest::read_blob_by_descriptor_async,
    remote_transport::{bounded_map, RemoteTransport},
    LocalArtifact, LocalManifest,
};
use anyhow::Context;
use oci_client::RegistryOperation;
use oci_spec::image::Descriptor;
use std::collections::HashMap;

const BLOB_TRANSFER_CONCURRENCY: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PushBlobOutcome {
    Skipped,
    Transferred,
}

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
        transport.auth(self.image_name())?;
        let outcomes = transport.block_on(async {
            bounded_map(
                unique_blob_descriptors,
                BLOB_TRANSFER_CONCURRENCY,
                |descriptor| {
                    let transport = &transport;
                    push_descriptor_if_missing(
                        descriptor,
                        move |digest| async move {
                            transport
                                .blob_exists_async(self.image_name(), &digest)
                                .await
                                .with_context(|| format!("Failed while checking blob {digest}"))
                        },
                        |descriptor| async move {
                            read_blob_by_descriptor_async(self, &descriptor).await
                        },
                        move |digest, bytes| async move {
                            transport
                                .push_blob_async(self.image_name(), &digest, bytes)
                                .await
                                .with_context(|| format!("Failed while uploading blob {digest}"))
                        },
                    )
                },
            )
            .await
        })?;
        let checked = outcomes.len();
        let skipped = outcomes
            .iter()
            .filter(|outcome| **outcome == PushBlobOutcome::Skipped)
            .count();
        let transferred = checked - skipped;
        tracing::info!(
            checked,
            skipped,
            transferred,
            deduplicated,
            "Completed remote blob transfers"
        );

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

async fn push_descriptor_if_missing<Exists, ExistsFuture, Read, ReadFuture, Push, PushFuture>(
    descriptor: &Descriptor,
    blob_exists: Exists,
    read_blob: Read,
    push_blob: Push,
) -> crate::Result<PushBlobOutcome>
where
    Exists: FnOnce(String) -> ExistsFuture,
    ExistsFuture: std::future::Future<Output = crate::Result<bool>>,
    Read: FnOnce(Descriptor) -> ReadFuture,
    ReadFuture: std::future::Future<Output = crate::Result<Vec<u8>>>,
    Push: FnOnce(String, Vec<u8>) -> PushFuture,
    PushFuture: std::future::Future<Output = crate::Result<()>>,
{
    let digest = descriptor.digest().to_string();
    if blob_exists(digest.clone()).await? {
        tracing::debug!(%digest, "Skipping blob already present in remote");
        return Ok(PushBlobOutcome::Skipped);
    }

    // Blob loading happens after the existence check, inside the same bounded
    // future, so no more than BLOB_TRANSFER_CONCURRENCY buffers are resident.
    let bytes = read_blob(descriptor.clone()).await?;
    tracing::debug!(size = bytes.len(), %digest, "Pushing blob");
    push_blob(digest, bytes).await?;
    Ok(PushBlobOutcome::Transferred)
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
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
        str::FromStr,
    };

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
        let read_count = Cell::new(0);
        let push_count = Cell::new(0);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let push_count_ref = &push_count;

        let outcome = runtime
            .block_on(push_descriptor_if_missing(
                &present,
                |_| async { Ok(true) },
                |_| async {
                    read_count.set(read_count.get() + 1);
                    Ok(Vec::new())
                },
                move |_, _| async move {
                    push_count_ref.set(push_count_ref.get() + 1);
                    Ok(())
                },
            ))
            .unwrap();

        assert_eq!(outcome, PushBlobOutcome::Skipped);
        assert_eq!(read_count.get(), 0);
        assert_eq!(push_count.get(), 0);
    }

    #[test]
    fn missing_descriptor_is_read_and_uploaded_in_one_operation() {
        let missing = descriptor_for(b"missing");
        let events = Rc::new(RefCell::new(Vec::new()));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let check_events = Rc::clone(&events);
        let read_events = Rc::clone(&events);
        let push_events = Rc::clone(&events);

        let outcome = runtime
            .block_on(push_descriptor_if_missing(
                &missing,
                move |_| async move {
                    check_events.borrow_mut().push("check");
                    Ok(false)
                },
                move |_| async move {
                    read_events.borrow_mut().push("read");
                    Ok(b"missing".to_vec())
                },
                move |_, bytes| async move {
                    assert_eq!(bytes, b"missing");
                    push_events.borrow_mut().push("push");
                    Ok(())
                },
            ))
            .unwrap();

        assert_eq!(outcome, PushBlobOutcome::Transferred);
        assert_eq!(*events.borrow(), ["check", "read", "push"]);
    }
}
