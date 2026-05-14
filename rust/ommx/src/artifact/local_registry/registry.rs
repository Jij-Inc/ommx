use super::{
    import_legacy_local_registry, import_legacy_local_registry_ref,
    import_legacy_local_registry_ref_with_policy, import_legacy_local_registry_with_policy,
    FileBlobStore, LegacyImportReport, OciDirImport, RefConflictPolicy, RefUpdate,
    SqliteIndexStore,
};
use crate::artifact::{media_types, sha256_digest, stable_json_bytes, ImageRef};
use anyhow::{ensure, Context, Result};
use oci_spec::image::{Descriptor, DescriptorBuilder, Digest, ImageManifest, MediaType};
use std::path::{Path, PathBuf};
use std::str::FromStr;

#[derive(Debug)]
pub struct LocalRegistry {
    root: PathBuf,
    index: SqliteIndexStore,
    blobs: FileBlobStore,
}

impl LocalRegistry {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        let index = SqliteIndexStore::open_in_registry_root(&root)?;
        let blobs = FileBlobStore::open_in_registry_root(&root)?;
        Ok(Self { root, index, blobs })
    }

    pub fn open_default() -> Result<Self> {
        Self::open(crate::artifact::get_local_registry_root())
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn index(&self) -> &SqliteIndexStore {
        &self.index
    }

    pub fn blobs(&self) -> &FileBlobStore {
        &self.blobs
    }

    pub fn import_legacy_ref(&self, image_name: &ImageRef) -> Result<OciDirImport> {
        import_legacy_local_registry_ref(&self.index, &self.blobs, &self.root, image_name)
    }

    pub fn import_legacy_ref_with_policy(
        &self,
        image_name: &ImageRef,
        policy: RefConflictPolicy,
    ) -> Result<OciDirImport> {
        import_legacy_local_registry_ref_with_policy(
            &self.index,
            &self.blobs,
            &self.root,
            image_name,
            policy,
        )
    }

    pub fn import_legacy_layout(&self) -> Result<LegacyImportReport> {
        import_legacy_local_registry(&self.index, &self.blobs, &self.root)
    }

    pub fn import_legacy_layout_with_policy(
        &self,
        policy: RefConflictPolicy,
    ) -> Result<LegacyImportReport> {
        import_legacy_local_registry_with_policy(&self.index, &self.blobs, &self.root, policy)
    }

    pub fn resolve_image_name(&self, image_name: &ImageRef) -> Result<Option<Digest>> {
        self.index.resolve_image_name(image_name)
    }

    /// Synthesize a fresh anonymous image name keyed to this
    /// registry's `registry_id`. Format matches
    /// `LocalArtifactBuilder::new_anonymous` and the unnamed-archive
    /// import path: `<registry-id8>.ommx.local/anonymous:<timestamp>-<nonce>`.
    /// Each call returns a new name (the nonce differs); the structural
    /// predicates [`crate::artifact::is_anonymous_artifact_ref_name`]
    /// and [`crate::artifact::is_anonymous_artifact_tag`] match every
    /// name produced this way, so
    /// `ommx artifact prune-anonymous` cleans them uniformly.
    pub fn synthesize_anonymous_image_name(&self) -> Result<ImageRef> {
        let registry_id = self.index.registry_id()?;
        crate::artifact::anonymous_artifact_image_name(&registry_id)
    }

    /// List every SQLite ref whose `(name, reference)` matches the
    /// shape an anonymous artifact's image name would take:
    /// `<registry-id8>.ommx.local/anonymous` (8 lowercase hex chars
    /// prefix + suffix) for the name, and `YYYYMMDDTHHMMSS-<nonce>`
    /// (timestamp + 12-hex random suffix) for the reference. Both
    /// must match — a substring check on the suffix alone would
    /// over-match a human-pushed ref against a real mDNS host like
    /// `myhost.ommx.local/anonymous:v1`. Returned in
    /// `(name, reference)` order to match
    /// [`SqliteIndexStore::list_refs`].
    pub fn list_anonymous_artifact_refs(
        &self,
    ) -> Result<Vec<crate::artifact::local_registry::RefRecord>> {
        let all = self.index.list_refs(None)?;
        Ok(all
            .into_iter()
            .filter(|r| {
                crate::artifact::is_anonymous_artifact_ref_name(&r.name)
                    && crate::artifact::is_anonymous_artifact_tag(&r.reference)
            })
            .collect())
    }

    /// Bulk-delete every SQLite ref produced by
    /// [`crate::artifact::LocalArtifactBuilder::new_anonymous`].
    /// Returns the deleted records so callers (e.g. CLI
    /// `ommx artifact prune-anonymous`) can report what changed. The
    /// manifest / config / layer / blob CAS records the deleted refs
    /// pointed at are **not** touched; they become unreferenced rows
    /// reclaimable by a future GC sweep. This is intentional — the
    /// prune is cheap and the orphan reclamation is the slower /
    /// riskier operation.
    pub fn prune_anonymous_artifact_refs(
        &self,
    ) -> Result<Vec<crate::artifact::local_registry::RefRecord>> {
        let refs = self.list_anonymous_artifact_refs()?;
        for r in &refs {
            self.index.delete_ref(&r.name, &r.reference)?;
        }
        Ok(refs)
    }

    /// Publish an OCI Image Manifest to the SQLite Local Registry.
    /// Callers must write every config/layer blob referenced by the
    /// manifest into the BlobStore before calling this method. The
    /// registry serializes the manifest itself, writes that manifest
    /// blob, and publishes the resulting descriptor into the index.
    pub(crate) fn publish_artifact_manifest(
        &self,
        image_name: &ImageRef,
        manifest: &ImageManifest,
        policy: RefConflictPolicy,
    ) -> Result<(Descriptor, RefUpdate)> {
        Self::validate_manifest(manifest)?;
        self.ensure_manifest_dependencies_exist(manifest)?;
        let manifest_bytes = stable_json_bytes(manifest)?;
        let manifest_descriptor = Self::build_manifest_descriptor(&manifest_bytes)?;

        // Pre-check: under `KeepExisting`, return the conflict before
        // we waste any CAS writes. The atomic publish in stage 2
        // re-validates the same condition inside the SQLite
        // transaction, so concurrent racers can't slip through; this
        // is purely a fast path for the common single-writer case.
        if policy == RefConflictPolicy::KeepExisting {
            if let Some(existing_descriptor) = self.index.resolve_image_descriptor(image_name)? {
                if existing_descriptor.digest() != manifest_descriptor.digest() {
                    let incoming_manifest_digest = manifest_descriptor.digest().clone();
                    return Ok((
                        manifest_descriptor,
                        RefUpdate::Conflicted {
                            existing_manifest_digest: existing_descriptor.digest().clone(),
                            incoming_manifest_digest,
                        },
                    ));
                }
            }
        }

        self.stage_blob(&manifest_descriptor, &manifest_bytes)?;
        let ref_update =
            self.index
                .put_image_ref_with_policy(image_name, &manifest_descriptor, policy)?;
        Ok((manifest_descriptor, ref_update))
    }

    fn ensure_manifest_dependencies_exist(&self, manifest: &ImageManifest) -> Result<()> {
        self.ensure_blob_exists("Manifest config", manifest.config())?;
        for layer in manifest.layers() {
            self.ensure_blob_exists("Manifest layer", layer)?;
        }
        Ok(())
    }

    fn ensure_blob_exists(&self, label: &str, descriptor: &Descriptor) -> Result<()> {
        ensure!(
            self.blobs.exists(descriptor.digest())?,
            "{label} blob is missing from the BlobStore: {}",
            descriptor.digest()
        );
        Ok(())
    }

    /// Validate that the manifest carries the OMMX `artifactType`.
    fn validate_manifest(manifest: &ImageManifest) -> Result<()> {
        let artifact_type = manifest
            .artifact_type()
            .as_ref()
            .context("Manifest does not carry the OMMX `artifactType` field")?;
        ensure!(
            artifact_type == &MediaType::Other(media_types::V1_ARTIFACT_MEDIA_TYPE.to_string()),
            "Manifest `artifactType` must be `{}`, got `{}`",
            media_types::V1_ARTIFACT_MEDIA_TYPE,
            artifact_type,
        );
        Ok(())
    }

    fn build_manifest_descriptor(manifest_bytes: &[u8]) -> Result<Descriptor> {
        DescriptorBuilder::default()
            .media_type(MediaType::ImageManifest)
            .digest(
                Digest::from_str(&sha256_digest(manifest_bytes))
                    .context("Failed to parse manifest digest")?,
            )
            .size(manifest_bytes.len() as u64)
            .build()
            .context("Failed to build manifest descriptor")
    }

    /// CAS-write a descriptor's bytes and verify the concrete bytes
    /// match the descriptor the manifest will reference.
    pub(crate) fn stage_blob(&self, descriptor: &Descriptor, bytes: &[u8]) -> Result<()> {
        let digest = self.blobs.put_bytes(bytes)?;
        ensure!(
            &digest == descriptor.digest(),
            "Descriptor digest mismatch: descriptor={}, actual={}",
            descriptor.digest(),
            digest
        );
        ensure!(
            bytes.len() as u64 == descriptor.size(),
            "Descriptor size mismatch for {}: descriptor={}, actual={}",
            descriptor.digest(),
            descriptor.size(),
            bytes.len()
        );
        Ok(())
    }
}
