use super::{
    import_legacy_local_registry, import_legacy_local_registry_ref, replace_legacy_local_registry,
    replace_legacy_local_registry_ref, FileBlobStore, LegacyImportReport, OciDirImport, RefUpdate,
    SqliteIndexStore,
};
use crate::artifact::{media_types, sha256_digest, stable_json_bytes, ImageRef};
use anyhow::{ensure, Context, Result};
use oci_spec::image::{Descriptor, DescriptorBuilder, Digest, ImageManifest, MediaType};
use std::collections::HashMap;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::OnceLock;

static DEFAULT_LOCAL_REGISTRY: OnceLock<LocalRegistry> = OnceLock::new();

/// OCI descriptor whose referenced bytes are known to exist in the
/// referenced Local Registry's BlobStore.
///
/// This is an OMMX / Local Registry invariant, not an invariant of
/// [`oci_spec::image::Descriptor`] itself. Values are created only by
/// [`LocalRegistry`] operations that have written or verified the
/// content-addressed blob.
///
/// The invariant is tied to the concrete [`LocalRegistry`] instance,
/// not merely to an equivalent registry root path or SQLite database.
/// Re-opening the same directory yields a different `LocalRegistry`
/// instance, and descriptors from that instance are not treated as
/// stored in this one until they are explicitly verified or written
/// through this instance.
#[derive(Debug, Clone)]
pub struct StoredDescriptor<'reg> {
    registry: &'reg LocalRegistry,
    descriptor: Descriptor,
}

impl StoredDescriptor<'_> {
    pub(crate) fn is_stored_in(&self, registry: &LocalRegistry) -> bool {
        // This intentionally checks registry-instance identity. Two
        // LocalRegistry values may point at the same on-disk SQLite /
        // BlobStore root, but a StoredDescriptor is only proven stored
        // for the instance that created or verified it.
        std::ptr::eq(self.registry, registry)
    }

    fn into_inner(self) -> Descriptor {
        self.descriptor
    }
}

impl Deref for StoredDescriptor<'_> {
    type Target = Descriptor;

    fn deref(&self) -> &Self::Target {
        &self.descriptor
    }
}

impl From<StoredDescriptor<'_>> for Descriptor {
    fn from(value: StoredDescriptor<'_>) -> Self {
        value.into_inner()
    }
}

/// Sealed OMMX Artifact.
///
/// The inner descriptor is stored in this registry, and it is known to
/// be the root manifest descriptor produced by [`LocalRegistry::seal_artifact`].
#[derive(Debug, Clone)]
pub(crate) struct SealedArtifact<'reg>(StoredDescriptor<'reg>);

impl<'reg> Deref for SealedArtifact<'reg> {
    type Target = StoredDescriptor<'reg>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl SealedArtifact<'_> {
    fn is_stored_in(&self, registry: &LocalRegistry) -> bool {
        self.0.is_stored_in(registry)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct UnsealedArtifact<'reg> {
    artifact_type: MediaType,
    config: StoredDescriptor<'reg>,
    layers: Vec<StoredDescriptor<'reg>>,
    subject: Option<Descriptor>,
    annotations: HashMap<String, String>,
}

impl<'reg> UnsealedArtifact<'reg> {
    pub(crate) fn new(
        artifact_type: MediaType,
        config: StoredDescriptor<'reg>,
        layers: Vec<StoredDescriptor<'reg>>,
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

    fn ensure_stored_in(&self, registry: &LocalRegistry) -> Result<()> {
        ensure!(
            self.config.is_stored_in(registry),
            "Artifact config descriptor belongs to a different Local Registry"
        );
        ensure!(
            self.layers
                .iter()
                .all(|descriptor| descriptor.is_stored_in(registry)),
            "Artifact layer descriptor belongs to a different Local Registry"
        );
        Ok(())
    }
}

#[derive(Debug)]
pub struct LocalRegistry {
    root: PathBuf,
    index: SqliteIndexStore,
    blobs: FileBlobStore,
}

/// Temporary Local Registry for tests and examples.
///
/// The temporary directory is owned by this value and is deleted when
/// the value is dropped. Borrow the registry while the `TempLocalRegistry`
/// value is alive.
#[derive(Debug)]
pub struct TempLocalRegistry {
    registry: LocalRegistry,
    tempdir: tempfile::TempDir,
}

impl TempLocalRegistry {
    pub fn new() -> Result<Self> {
        let tempdir = tempfile::tempdir().context("Failed to create temporary Local Registry")?;
        let registry = LocalRegistry::open(tempdir.path())?;
        Ok(Self { registry, tempdir })
    }

    pub fn registry(&self) -> &LocalRegistry {
        &self.registry
    }

    pub fn path(&self) -> &Path {
        self.tempdir.path()
    }
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

    /// Return the process-wide default Local Registry.
    ///
    /// The default registry is opened lazily on the first call and then
    /// reused for the rest of the process. Call
    /// [`crate::artifact::set_local_registry_root`] before this method
    /// if a non-default root is needed.
    pub fn shared_default() -> Result<&'static Self> {
        if let Some(registry) = DEFAULT_LOCAL_REGISTRY.get() {
            return Ok(registry);
        }

        // OnceLock::get_or_try_init is still unstable on the supported
        // toolchain. This open-then-set sequence can briefly open two
        // SQLite connections if multiple threads race on the first
        // call, but only one registry is retained. Replace this with
        // get_or_try_init once it is stable.
        let registry = Self::open_default()?;
        let _ = DEFAULT_LOCAL_REGISTRY.set(registry);
        Ok(DEFAULT_LOCAL_REGISTRY
            .get()
            .expect("default Local Registry was initialized"))
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

    pub fn get_blob(&self, descriptor: &StoredDescriptor<'_>) -> Result<Vec<u8>> {
        ensure!(
            descriptor.is_stored_in(self),
            "Descriptor {} is not stored in this Local Registry",
            descriptor.digest()
        );
        let bytes = self.blobs.read_bytes(descriptor.digest())?;
        ensure!(
            bytes.len() as u64 == descriptor.size(),
            "Descriptor size mismatch for {}: descriptor={}, actual={}",
            descriptor.digest(),
            descriptor.size(),
            bytes.len()
        );
        Ok(bytes)
    }

    pub fn import_legacy_ref(&self, image_name: &ImageRef) -> Result<OciDirImport> {
        import_legacy_local_registry_ref(&self.index, &self.blobs, &self.root, image_name)
    }

    pub fn replace_legacy_ref(&self, image_name: &ImageRef) -> Result<OciDirImport> {
        replace_legacy_local_registry_ref(&self.index, &self.blobs, &self.root, image_name)
    }

    pub fn import_legacy_layout(&self) -> Result<LegacyImportReport> {
        import_legacy_local_registry(&self.index, &self.blobs, &self.root)
    }

    pub fn replace_legacy_layout(&self) -> Result<LegacyImportReport> {
        replace_legacy_local_registry(&self.index, &self.blobs, &self.root)
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

    /// Synthesize a fresh anonymous Experiment image name keyed to
    /// this registry's `registry_id`.
    ///
    /// Format:
    /// `<registry-id8>.ommx.local/experiment:<timestamp>-<nonce>`.
    /// This keeps unnamed experiments under a distinct local
    /// repository while preserving the same non-colliding tag shape as
    /// anonymous artifacts.
    pub fn synthesize_anonymous_experiment_image_name(&self) -> Result<ImageRef> {
        let registry_id = self.index.registry_id()?;
        crate::artifact::anonymous_local_image_name(&registry_id, "experiment")
            .with_context(|| "Failed to synthesise anonymous experiment image name")
    }

    /// Synthesize a fresh local ref for an Experiment recovery artifact.
    ///
    /// Format:
    /// `<registry-id8>.ommx.local/crashed:<timestamp>-<nonce>`.
    /// Recovery artifacts are separate from the requested Experiment ref so
    /// a failed context-manager exit never advances the success tag.
    pub fn synthesize_crashed_experiment_image_name(&self) -> Result<ImageRef> {
        let registry_id = self.index.registry_id()?;
        crate::artifact::anonymous_local_image_name(&registry_id, "crashed")
            .with_context(|| "Failed to synthesise crashed experiment image name")
    }

    /// Synthesize a fresh local ref for a rolling Experiment autosave artifact.
    ///
    /// Format:
    /// `<registry-id8>.ommx.local/autosave:<timestamp>-<nonce>`.
    /// The ref is generated once per Experiment session and moved forward as
    /// closed Runs update the latest checkpoint.
    pub fn synthesize_autosave_experiment_image_name(&self) -> Result<ImageRef> {
        let registry_id = self.index.registry_id()?;
        crate::artifact::anonymous_local_image_name(&registry_id, "autosave")
            .with_context(|| "Failed to synthesise autosave experiment image name")
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
    pub(crate) fn seal_artifact<'reg>(
        &'reg self,
        artifact: UnsealedArtifact<'reg>,
    ) -> Result<SealedArtifact<'reg>> {
        artifact.ensure_stored_in(self)?;
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
        sealed_artifact: &SealedArtifact<'_>,
    ) -> Result<RefUpdate> {
        ensure!(
            sealed_artifact.is_stored_in(self),
            "Sealed artifact descriptor belongs to a different Local Registry"
        );
        self.index.publish_image_ref(image_name, &sealed_artifact.0)
    }

    /// Publish an already-stored root manifest descriptor under an image ref.
    ///
    /// This is used when adding another local name for an existing artifact.
    /// It is an IndexStore operation only: no payload blobs or manifest bytes
    /// are rewritten.
    pub(crate) fn publish_stored_manifest_ref(
        &self,
        image_name: &ImageRef,
        manifest: &StoredDescriptor<'_>,
    ) -> Result<RefUpdate> {
        ensure!(
            manifest.is_stored_in(self),
            "Manifest descriptor belongs to a different Local Registry"
        );
        self.index.publish_image_ref(image_name, manifest)
    }

    /// Replace the ref target with a sealed root manifest descriptor.
    ///
    /// This is an IndexStore operation only. It does not write payload
    /// blobs or manifest bytes.
    pub(crate) fn replace_manifest_ref(
        &self,
        image_name: &ImageRef,
        sealed_artifact: &SealedArtifact<'_>,
    ) -> Result<RefUpdate> {
        ensure!(
            sealed_artifact.is_stored_in(self),
            "Sealed artifact descriptor belongs to a different Local Registry"
        );
        self.index.replace_image_ref(image_name, &sealed_artifact.0)
    }

    /// Delete a local manifest ref. Content-addressed blobs are not removed.
    pub(crate) fn delete_manifest_ref(&self, image_name: &ImageRef) -> Result<bool> {
        self.index
            .delete_ref(&image_name.repository_key(), image_name.reference())
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

    /// Store bytes as an OCI layer descriptor in this registry's
    /// BlobStore. The descriptor carries the supplied media type and
    /// annotations, and its digest / size are derived from `bytes`.
    pub(crate) fn store_layer_blob(
        &self,
        media_type: MediaType,
        bytes: &[u8],
        annotations: HashMap<String, String>,
    ) -> Result<StoredDescriptor<'_>> {
        let digest =
            Digest::from_str(&sha256_digest(bytes)).context("Failed to parse layer blob digest")?;
        let descriptor = DescriptorBuilder::default()
            .media_type(media_type)
            .digest(digest)
            .size(bytes.len() as u64)
            .annotations(annotations)
            .build()
            .context("Failed to build layer descriptor")?;
        self.store_blob(descriptor, bytes)
    }

    /// Serialize `value` as JSON and store it as an OCI layer blob in
    /// this registry.
    pub(crate) fn store_json_layer_blob(
        &self,
        media_type: MediaType,
        value: &impl serde::Serialize,
        annotations: HashMap<String, String>,
    ) -> Result<StoredDescriptor<'_>> {
        let bytes = serde_json::to_vec(value).context("Failed to encode JSON layer")?;
        self.store_layer_blob(media_type, &bytes, annotations)
    }

    /// Serialize `value` as JSON and store it as a generic OCI blob
    /// descriptor without layer annotations.
    pub(crate) fn store_json_blob(
        &self,
        media_type: MediaType,
        value: &impl serde::Serialize,
    ) -> Result<StoredDescriptor<'_>> {
        let bytes = serde_json::to_vec(value).context("Failed to encode JSON blob")?;
        let digest =
            Digest::from_str(&sha256_digest(&bytes)).context("Failed to parse JSON blob digest")?;
        let descriptor = DescriptorBuilder::default()
            .media_type(media_type)
            .digest(digest)
            .size(bytes.len() as u64)
            .build()
            .context("Failed to build JSON blob descriptor")?;
        self.store_blob(descriptor, &bytes)
    }

    /// Store a descriptor's bytes as a content-addressed blob and
    /// verify the concrete bytes match the descriptor.
    pub(crate) fn store_blob(
        &self,
        descriptor: Descriptor,
        bytes: &[u8],
    ) -> Result<StoredDescriptor<'_>> {
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
        Ok(StoredDescriptor {
            registry: self,
            descriptor,
        })
    }

    /// Verify that the blob referenced by `descriptor` exists in this
    /// registry and promote it to a [`StoredDescriptor`].
    pub(crate) fn stored_descriptor(&self, descriptor: Descriptor) -> Result<StoredDescriptor<'_>> {
        let size = self.blobs.size(descriptor.digest())?;
        ensure!(
            size == descriptor.size(),
            "Descriptor size mismatch for {}: descriptor={}, actual={}",
            descriptor.digest(),
            descriptor.size(),
            size
        );
        Ok(StoredDescriptor {
            registry: self,
            descriptor,
        })
    }
}
