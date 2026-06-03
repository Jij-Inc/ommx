//! Remote OCI registry → v3 SQLite Local Registry import.
//!
//! ## Implementation shape
//!
//! Single network-to-SQLite pass through
//! `remote_transport::RemoteTransport`:
//!
//! 1. Pre-pull SQLite check short-circuits the network fetch when the
//!    registry already resolves `image_name` to a manifest digest **and**
//!    the manifest, config, and layer blobs are present in the registry.
//!    The method returns an [`OciDirImport`] with
//!    [`super::super::RefUpdate::Unchanged`] without touching the
//!    network. The blob-presence probe distinguishes a healthy hit from
//!    a half-populated registry (e.g. manual CAS-file deletion,
//!    interrupted import): the latter falls through to a fresh pull
//!    with a `tracing::warn!` event rather than handing back a stale
//!    `Unchanged` that would surface as an opaque `get_blob` failure
//!    later. Same cache-hit semantics the v2-era legacy dir cache
//!    offered, expressed against the canonical SQLite ref store.
//! 2. Open a `RemoteTransport`, authenticate for `Pull`, fetch the
//!    manifest bytes verbatim, then walk the manifest's config +
//!    layer descriptors. Each blob is pulled into memory and written
//!    to the registry. The manifest is stored last so it sits behind
//!    its blobs (matching the OCI distribution
//!    publish order).
//! 3. SQLite publishes only the manifest descriptor under the requested
//!    `image_name`. A crash between blob writes and ref publish leaves
//!    orphan CAS bytes (recovered by GC, not visible through the index).
//!
//! v3 has no on-disk OCI Image Layout intermediate for pulls — SQLite
//! plus registry-owned CAS files are the sole post-import home of the bytes.
//!
//! Feature-gated behind `remote-artifact` because the `RemoteTransport`
//! is, and because this is the only place in `local_registry` that
//! touches the network.

use super::super::super::RefUpdate;
use super::super::LocalRegistry;
use super::oci_dir::OciDirImport;
use crate::artifact::{
    media_types, remote_transport::RemoteTransport, ImageRef, OCI_IMAGE_MANIFEST_MEDIA_TYPE,
};
use anyhow::{Context, Result};
use oci_client::RegistryOperation;
use oci_spec::image::{Descriptor, DescriptorBuilder, Digest, ImageManifest, MediaType};
use std::str::FromStr;

impl LocalRegistry {
    /// Pull `image_name` from its remote registry into this Local Registry.
    ///
    /// If the registry already resolves `image_name` to a manifest whose
    /// config and layer blobs are also present in the registry, the network
    /// fetch is skipped and the method returns [`OciDirImport`] with
    /// [`RefUpdate::Unchanged`].
    ///
    /// Otherwise the manifest and each blob are pulled through
    /// `RemoteTransport` straight into the registry, and a SQLite transaction
    /// publishes the ref descriptor. There is no on-disk OCI Image Layout
    /// intermediate.
    pub fn pull_image(&self, image_name: &ImageRef) -> Result<OciDirImport> {
        RemotePull::new(self, image_name).run()
    }
}

struct RemotePull<'reg, 'name> {
    registry: &'reg LocalRegistry,
    image_name: &'name ImageRef,
}

impl<'reg, 'name> RemotePull<'reg, 'name> {
    fn new(registry: &'reg LocalRegistry, image_name: &'name ImageRef) -> Self {
        Self {
            registry,
            image_name,
        }
    }

    fn run(&self) -> Result<OciDirImport> {
        if let Some(cached) = self.cached_ref()? {
            return Ok(cached);
        }

        let transport = RemoteTransport::new(self.image_name)?;
        transport.auth_for(self.image_name, RegistryOperation::Pull)?;

        tracing::info!("Pulling {} into the v3 Local Registry", self.image_name);
        let (manifest_bytes, manifest_digest) = transport.pull_manifest_raw(
            self.image_name,
            &[
                OCI_IMAGE_MANIFEST_MEDIA_TYPE,
                "application/vnd.oci.image.index.v1+json",
            ],
        )?;
        let manifest: ImageManifest = serde_json::from_slice(&manifest_bytes)
            .context("Failed to parse OCI image manifest pulled from the remote registry")?;
        Self::ensure_ommx_image_manifest(&manifest)?;

        let manifest_digest = Digest::from_str(&manifest_digest)
            .with_context(|| format!("Invalid remote manifest digest: {manifest_digest}"))?;
        let manifest_descriptor = DescriptorBuilder::default()
            .media_type(MediaType::ImageManifest)
            .digest(manifest_digest.clone())
            .size(manifest_bytes.len() as u64)
            .build()
            .context("Failed to build remote manifest descriptor")?;

        self.pull_descriptor_blob(&transport, manifest.config())?;

        for layer in manifest.layers() {
            self.pull_descriptor_blob(&transport, layer)?;
        }

        self.store_manifest_blob(&manifest_descriptor, &manifest_bytes, &manifest_digest)?;

        let ref_update = self
            .registry
            .index
            .publish_image_ref(self.image_name, &manifest_descriptor)?;
        self.reject_conflicting_ref(&ref_update)?;

        Ok(OciDirImport {
            manifest_digest,
            image_name: self.image_name.clone(),
            ref_update,
        })
    }

    fn cached_ref(&self) -> Result<Option<OciDirImport>> {
        let Some(manifest_descriptor) = self
            .registry
            .index
            .resolve_image_descriptor(self.image_name)?
        else {
            return Ok(None);
        };
        let manifest_digest = manifest_descriptor.digest().clone();
        if self.cached_manifest_closure_is_present(&manifest_descriptor)? {
            return Ok(Some(OciDirImport {
                manifest_digest,
                image_name: self.image_name.clone(),
                ref_update: RefUpdate::Unchanged,
            }));
        }
        tracing::warn!(
            "SQLite ref resolves {} → {manifest_digest}, but the manifest closure \
             is incomplete in the registry; falling through to a fresh remote pull \
             to repopulate the registry",
            self.image_name,
        );
        Ok(None)
    }

    fn cached_manifest_closure_is_present(&self, manifest_descriptor: &Descriptor) -> Result<bool> {
        let manifest_digest = manifest_descriptor.digest();
        if !self.registry.contains_blob(manifest_digest)? {
            return Ok(false);
        }
        let manifest_size = self.registry.blob_size(manifest_digest)?;
        if manifest_size != manifest_descriptor.size() {
            tracing::warn!(
                "SQLite ref resolves {} → {}, but the manifest blob size is {}; \
                 expected {}",
                self.image_name,
                manifest_digest,
                manifest_size,
                manifest_descriptor.size(),
            );
            return Ok(false);
        }

        let manifest_bytes = self.registry.read_blob(manifest_digest)?;
        let manifest: ImageManifest = serde_json::from_slice(&manifest_bytes)
            .with_context(|| format!("Failed to parse cached manifest {manifest_digest}"))?;
        Self::ensure_ommx_image_manifest(&manifest)?;

        if !self.cached_descriptor_blob_is_present(manifest.config())? {
            return Ok(false);
        }
        for layer in manifest.layers() {
            if !self.cached_descriptor_blob_is_present(layer)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn cached_descriptor_blob_is_present(&self, descriptor: &Descriptor) -> Result<bool> {
        if !self.registry.contains_blob(descriptor.digest())? {
            return Ok(false);
        }
        let size = self.registry.blob_size(descriptor.digest())?;
        if size != descriptor.size() {
            tracing::warn!(
                "Cached blob {} has size {}; expected {}",
                descriptor.digest(),
                size,
                descriptor.size(),
            );
            return Ok(false);
        }
        Ok(true)
    }

    /// Validate that the remote manifest is an OCI Image Manifest carrying
    /// the OMMX `artifactType` field. v3 SDK accepts only this format;
    /// callers that need to publish other artifacts go through the OCI
    /// distribution APIs directly.
    fn ensure_ommx_image_manifest(manifest: &ImageManifest) -> Result<()> {
        let artifact_type = manifest
            .artifact_type()
            .as_ref()
            .context("Remote manifest is not an OMMX artifact: artifactType is missing")?;
        anyhow::ensure!(
            artifact_type == &media_types::v1_artifact(),
            "Remote manifest is not an OMMX artifact: {artifact_type}"
        );
        if let Some(media_type) = manifest.media_type() {
            anyhow::ensure!(
                media_type == &MediaType::ImageManifest,
                "Remote manifest media type must be OCI Image Manifest, got {media_type}"
            );
        }
        Ok(())
    }

    /// Pull a single descriptor's blob from the registry, write it into
    /// the registry under its content digest, and verify the written
    /// bytes match the descriptor.
    fn pull_descriptor_blob(
        &self,
        transport: &RemoteTransport,
        descriptor: &Descriptor,
    ) -> Result<()> {
        let digest = descriptor.digest().to_string();
        // The manifest descriptor's `size` bounds the network read: the
        // transport's pull helper allocates from this value (not from the
        // registry-reported `Content-Length`) and aborts the chunk loop if
        // the registry serves more bytes than declared.
        let bytes = transport.pull_blob_to_vec(self.image_name, &digest, descriptor.size())?;
        anyhow::ensure!(
            bytes.len() as u64 == descriptor.size(),
            "Blob size mismatch for {digest}: descriptor={}, actual={}",
            descriptor.size(),
            bytes.len()
        );
        self.registry.store_blob(descriptor.clone(), &bytes)?;
        Ok(())
    }

    /// Store the manifest bytes into the registry under their
    /// registry-reported digest. The check that local sha256 matches the
    /// registry-reported digest doubles as an integrity probe on the
    /// manifest body: an upstream proxy that rewrote the manifest would
    /// surface here instead of producing an artifact whose published ref
    /// points at a manifest blob the registry does not actually serve.
    fn store_manifest_blob(
        &self,
        descriptor: &Descriptor,
        manifest_bytes: &[u8],
        expected_digest: &Digest,
    ) -> Result<()> {
        anyhow::ensure!(
            descriptor.digest() == expected_digest,
            "Manifest descriptor digest mismatch: descriptor={}, registry reported {}",
            descriptor.digest(),
            expected_digest
        );
        self.registry
            .store_blob(descriptor.clone(), manifest_bytes)?;
        Ok(())
    }

    fn reject_conflicting_ref(&self, ref_update: &RefUpdate) -> Result<()> {
        // Surface a ref conflict as `Err` rather than `Ok(Conflicted)`:
        // callers (Python `Artifact.load`, CLI `ommx pull`, dataset
        // loaders) treat a successful return as "the freshly pulled bytes
        // are now resident under `image_name`". Under publish semantics,
        // a conflict means the SQLite ref still points at the *prior*
        // manifest digest; opening `LocalArtifact` after that would
        // silently surface the local cache, not the remote bytes. Forcing
        // an explicit error makes the caller choose an explicit replace
        // operation or abort.
        if let RefUpdate::Conflicted {
            existing_manifest_digest,
            incoming_manifest_digest,
        } = ref_update
        {
            anyhow::bail!(
                "Local registry ref conflict for {}: existing manifest \
                 {existing_manifest_digest}, incoming manifest {incoming_manifest_digest}. \
                 The remote serves a different manifest than the one cached locally.",
                self.image_name
            );
        }
        Ok(())
    }
}
