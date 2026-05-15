use super::{
    import_legacy_local_registry, import_legacy_local_registry_ref,
    import_legacy_local_registry_ref_with_policy, import_legacy_local_registry_with_policy,
    FileBlobStore, LegacyImportReport, OciDirImport, RefConflictPolicy, RefUpdate,
    SqliteIndexStore,
};
use crate::artifact::{media_types, sha256_digest, stable_json_bytes, ImageRef};
use anyhow::{ensure, Context, Result};
use oci_spec::image::{Descriptor, DescriptorBuilder, Digest, ImageManifest, MediaType};
use std::collections::HashMap;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// OCI descriptor whose referenced bytes are known to exist in this
/// Local Registry's BlobStore.
///
/// This is an OMMX / Local Registry invariant, not an invariant of
/// [`oci_spec::image::Descriptor`] itself. Values are created only by
/// [`LocalRegistry`] operations that have written or verified the
/// content-addressed blob.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredDescriptor(Descriptor);

impl StoredDescriptor {
    fn into_inner(self) -> Descriptor {
        self.0
    }
}

impl Deref for StoredDescriptor {
    type Target = Descriptor;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<StoredDescriptor> for Descriptor {
    fn from(value: StoredDescriptor) -> Self {
        value.into_inner()
    }
}

/// Sealed OMMX Artifact.
///
/// The inner descriptor is stored in this registry, and it is known to
/// be the root manifest descriptor produced by [`LocalRegistry::seal_artifact`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SealedArtifact(StoredDescriptor);

impl Deref for SealedArtifact {
    type Target = StoredDescriptor;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Clone)]
pub(crate) struct UnsealedArtifact {
    artifact_type: MediaType,
    config: StoredDescriptor,
    layers: Vec<StoredDescriptor>,
    subject: Option<Descriptor>,
    annotations: HashMap<String, String>,
}

impl UnsealedArtifact {
    pub(crate) fn new(
        artifact_type: MediaType,
        config: StoredDescriptor,
        layers: Vec<StoredDescriptor>,
        subject: Option<Descriptor>,
        annotations: HashMap<String, String>,
    ) -> Self {
        Self {
            artifact_type,
            config,
            layers,
            subject,
            annotations,
        }
    }

    pub(crate) fn into_oci_image_manifest(self) -> Result<ImageManifest> {
        let config: Descriptor = self.config.into();
        let mut builder = oci_spec::image::ImageManifestBuilder::default()
            .schema_version(2u32)
            .artifact_type(self.artifact_type)
            .config(config)
            .layers(self.layers.into_iter().map(Into::into).collect::<Vec<_>>());
        if let Some(subject) = self.subject {
            builder = builder.subject(subject);
        }
        if !self.annotations.is_empty() {
            builder = builder.annotations(self.annotations);
        }
        builder
            .build()
            .context("Failed to build OCI image manifest")
    }
}

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
    /// `ArtifactDraft::new_anonymous` and the unnamed-archive
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
    /// [`crate::artifact::ArtifactDraft::new_anonymous`].
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

    /// Seal an unsealed OMMX Artifact manifest into the BlobStore.
    ///
    /// The manifest's config/layers are represented as
    /// [`StoredDescriptor`] before this method is called, so sealing
    /// does not re-validate dependency blob existence. It serializes
    /// and stores only the root manifest blob, yielding its root
    /// [`SealedArtifact`].
    pub(crate) fn seal_artifact(&self, artifact: UnsealedArtifact) -> Result<SealedArtifact> {
        let manifest = artifact.into_oci_image_manifest()?;
        Self::validate_manifest(&manifest)?;
        let manifest_bytes = stable_json_bytes(&manifest)?;
        let manifest_descriptor = Self::build_manifest_descriptor(&manifest_bytes)?;
        let stored_manifest = self.store_blob(manifest_descriptor, &manifest_bytes)?;
        Ok(SealedArtifact(stored_manifest))
    }

    /// Publish a sealed root manifest descriptor under an image ref.
    ///
    /// This is an IndexStore operation only. It does not write payload
    /// blobs or manifest bytes.
    pub(crate) fn publish_manifest_ref(
        &self,
        image_name: &ImageRef,
        sealed_artifact: &SealedArtifact,
        policy: RefConflictPolicy,
    ) -> Result<RefUpdate> {
        self.index
            .put_image_ref_with_policy(image_name, &sealed_artifact.0, policy)
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

    /// Store a descriptor's bytes as a content-addressed blob and
    /// verify the concrete bytes match the descriptor.
    pub(crate) fn store_blob(
        &self,
        descriptor: Descriptor,
        bytes: &[u8],
    ) -> Result<StoredDescriptor> {
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
        Ok(StoredDescriptor(descriptor))
    }
}
