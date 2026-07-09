use super::index::SqliteIndexStore;
use super::{ExperimentManifestRecord, ExperimentRefRecord, RefUpdate};
use crate::artifact::{
    media_types::{self, RootPayloadVersion},
    sha256_digest, stable_json_bytes, ImageRef, LocalArtifact,
};
use anyhow::{ensure, Context, Result};
use oci_spec::image::{Descriptor, DescriptorBuilder, Digest, ImageManifest, MediaType};
use std::collections::{BTreeSet, HashMap};
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::OnceLock;

mod blob;
mod gc;
mod import;

use blob::{BlobRecord, DeleteBlobOutcome, FileBlobStore};
pub use gc::{
    GcBlob, GcDeleteReport, GcInvalidManifest, GcMissingBlob, GcOptions, GcReferenceKind, GcReport,
    GcRoot,
};
pub use import::{ArchiveInspectView, LegacyImportReport, OciDirImport, OciDirRef};

static DEFAULT_LOCAL_REGISTRY: OnceLock<LocalRegistry> = OnceLock::new();
const EXPERIMENT_CHECKPOINT_REPOSITORY: &str = "checkpoint";
const FILE_BLOB_STORE_DIR_NAME: &str = "blobs";

/// OCI descriptor whose referenced bytes are known to exist in the
/// referenced Local Registry.
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
    /// Ensure this descriptor has the expected OCI media type before a typed
    /// decoder reads its blob.
    pub fn ensure_media_type(&self, expected: &MediaType) -> Result<()> {
        let actual = self.media_type();
        ensure!(
            actual == expected,
            "Expected media type '{expected}', got '{actual}'"
        );
        Ok(())
    }

    /// Check registry-instance identity for crate-internal artifact handles.
    ///
    /// This is crate-visible because `LocalArtifact` and Experiment state live
    /// in sibling top-level modules but still need to enforce the Local Registry
    /// capability invariant before reading or publishing blobs.
    pub(crate) fn is_stored_in(&self, registry: &LocalRegistry) -> bool {
        // This intentionally checks registry-instance identity. Two
        // LocalRegistry values may point at the same on-disk root, but a
        // StoredDescriptor is only proven stored for the instance that created
        // or verified it.
        std::ptr::eq(self.registry, registry)
    }

    /// Crate-internal Experiment helpers need the proven registry to read blobs.
    pub(crate) fn registry(&self) -> &LocalRegistry {
        self.registry
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
/// Crate-visible because `artifact` / `experiment` publish flows live outside
/// the `local_registry` module while the sealed descriptor invariant belongs
/// to `LocalRegistry`.
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

/// Unsealed artifact payload prepared by crate-internal artifact builders.
///
/// This remains crate-visible so top-level `artifact` / `experiment` modules
/// can assemble config/layer descriptors while `LocalRegistry` remains the only
/// module that seals them into a root manifest blob.
#[derive(Debug, Clone)]
pub(crate) struct UnsealedArtifact<'reg> {
    artifact_type: MediaType,
    config: StoredDescriptor<'reg>,
    layers: Vec<StoredDescriptor<'reg>>,
    subject: Option<Descriptor>,
    annotations: HashMap<String, String>,
}

impl<'reg> UnsealedArtifact<'reg> {
    /// Build an unsealed manifest payload from registry-owned descriptors.
    ///
    /// Crate-visible for `artifact` / `experiment` builders; sealing and
    /// manifest validation still happen only through `LocalRegistry`.
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

    /// Consume the payload into an OCI Image Manifest for registry sealing.
    ///
    /// Crate-visible only to keep the type self-contained for tests and
    /// registry-owned sealing; external callers should use artifact builders.
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
        let blobs = FileBlobStore::new(root.join(FILE_BLOB_STORE_DIR_NAME))?;
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

    pub fn get_blob(&self, descriptor: &StoredDescriptor<'_>) -> Result<Vec<u8>> {
        ensure!(
            descriptor.is_stored_in(self),
            "Descriptor {} is not stored in this Local Registry",
            descriptor.digest()
        );
        let bytes = self.read_blob(descriptor.digest())?;
        ensure!(
            bytes.len() as u64 == descriptor.size(),
            "Descriptor size mismatch for {}: descriptor={}, actual={}",
            descriptor.digest(),
            descriptor.size(),
            bytes.len()
        );
        Ok(bytes)
    }

    pub fn get_instance_layer(&self, descriptor: &StoredDescriptor<'_>) -> Result<crate::Instance> {
        let payload_version = media_types::instance_payload_version(descriptor.media_type())?;
        let bytes = self.get_blob(descriptor)?;
        let annotations = descriptor
            .annotations()
            .as_ref()
            .cloned()
            .unwrap_or_default();
        let mut instance = match payload_version {
            RootPayloadVersion::V1 => crate::Instance::from_v1_bytes(&bytes)?,
            RootPayloadVersion::V2 => crate::Instance::from_v2_bytes(&bytes)?,
        };
        crate::FlatAnnotations::merge_annotations(&mut instance, &annotations);
        Ok(instance)
    }

    pub fn get_parametric_instance_layer(
        &self,
        descriptor: &StoredDescriptor<'_>,
    ) -> Result<crate::ParametricInstance> {
        let payload_version =
            media_types::parametric_instance_payload_version(descriptor.media_type())?;
        let bytes = self.get_blob(descriptor)?;
        let annotations = descriptor
            .annotations()
            .as_ref()
            .cloned()
            .unwrap_or_default();
        let mut instance = match payload_version {
            RootPayloadVersion::V1 => crate::ParametricInstance::from_v1_bytes(&bytes)?,
            RootPayloadVersion::V2 => crate::ParametricInstance::from_v2_bytes(&bytes)?,
        };
        crate::FlatAnnotations::merge_annotations(&mut instance, &annotations);
        Ok(instance)
    }

    pub fn get_solution_layer(&self, descriptor: &StoredDescriptor<'_>) -> Result<crate::Solution> {
        let payload_version = media_types::solution_payload_version(descriptor.media_type())?;
        let bytes = self.get_blob(descriptor)?;
        let annotations = descriptor
            .annotations()
            .as_ref()
            .cloned()
            .unwrap_or_default();
        let mut solution = match payload_version {
            RootPayloadVersion::V1 => crate::Solution::from_v1_bytes(&bytes)?,
            RootPayloadVersion::V2 => crate::Solution::from_v2_bytes(&bytes)?,
        };
        crate::FlatAnnotations::merge_annotations(&mut solution, &annotations);
        Ok(solution)
    }

    pub fn get_sample_set_layer(
        &self,
        descriptor: &StoredDescriptor<'_>,
    ) -> Result<crate::SampleSet> {
        let payload_version = media_types::sample_set_payload_version(descriptor.media_type())?;
        let bytes = self.get_blob(descriptor)?;
        let annotations = descriptor
            .annotations()
            .as_ref()
            .cloned()
            .unwrap_or_default();
        let mut sample_set = match payload_version {
            RootPayloadVersion::V1 => crate::SampleSet::from_v1_bytes(&bytes)?,
            RootPayloadVersion::V2 => crate::SampleSet::from_v2_bytes(&bytes)?,
        };
        crate::FlatAnnotations::merge_annotations(&mut sample_set, &annotations);
        Ok(sample_set)
    }

    pub fn store_instance_layer(&self, instance: &crate::Instance) -> Result<StoredDescriptor<'_>> {
        self.store_layer_blob(
            media_types::v2_instance(),
            &instance.to_v2_bytes(),
            crate::FlatAnnotations::flat_annotations(instance),
        )
    }

    pub fn store_parametric_instance_layer(
        &self,
        instance: &crate::ParametricInstance,
    ) -> Result<StoredDescriptor<'_>> {
        self.store_layer_blob(
            media_types::v2_parametric_instance(),
            &instance.to_v2_bytes(),
            crate::FlatAnnotations::flat_annotations(instance),
        )
    }

    pub fn store_solution_layer(&self, solution: &crate::Solution) -> Result<StoredDescriptor<'_>> {
        self.store_layer_blob(
            media_types::v2_solution(),
            &solution.to_v2_bytes(),
            crate::FlatAnnotations::flat_annotations(solution),
        )
    }

    pub fn store_sample_set_layer(
        &self,
        sample_set: &crate::SampleSet,
    ) -> Result<StoredDescriptor<'_>> {
        self.store_layer_blob(
            media_types::v2_sample_set(),
            &sample_set.to_v2_bytes(),
            crate::FlatAnnotations::flat_annotations(sample_set),
        )
    }

    pub fn resolve_image_name(&self, image_name: &ImageRef) -> Result<Option<Digest>> {
        self.index.resolve_image_name(image_name)
    }

    /// Per-registry stable identifier used to derive anonymous local refs.
    ///
    /// Crate-visible because top-level artifact builders construct anonymous
    /// names, while the identifier itself is Local Registry metadata.
    pub(crate) fn registry_id(&self) -> Result<String> {
        self.index.registry_id()
    }

    /// Synthesize a fresh anonymous image name keyed to this
    /// registry's `registry_id`. Format matches
    /// `ArtifactDraft::new_anonymous` and the unnamed-archive
    /// import path: `<registry-id8>.ommx.local/anonymous:<timestamp>-<nonce>`.
    /// Each call returns a new name (the nonce differs); the structural
    /// predicates [`crate::artifact::is_anonymous_artifact_ref_name`]
    /// and [`crate::artifact::is_anonymous_artifact_tag`] match every
    /// name produced this way, so
    /// `ommx prune-anonymous` cleans them uniformly.
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

    /// Deterministic local ref for an Experiment checkpoint artifact.
    ///
    /// Format:
    /// `<registry-id8>.ommx.local/checkpoint:<sha256-requested-image-name>`.
    /// Checkpoint artifacts are separate from the requested Experiment ref so
    /// autosave and recovery materialization never advance the success tag.
    /// Crate-visible because Experiment state is a sibling top-level module,
    /// while checkpoint refs are part of the Local Registry naming scheme.
    pub(crate) fn experiment_checkpoint_image_name(
        &self,
        requested_image_name: &ImageRef,
    ) -> Result<ImageRef> {
        let registry_id = self.index.registry_id()?;
        let repository_key = crate::artifact::anonymous_local_repository_key(
            &registry_id,
            EXPERIMENT_CHECKPOINT_REPOSITORY,
        )?;
        let digest = sha256_digest(requested_image_name.to_string().as_bytes());
        let tag = digest
            .strip_prefix("sha256:")
            .expect("sha256_digest returns a sha256-prefixed digest");
        ImageRef::parse(&format!("{repository_key}:{tag}")).with_context(|| {
            format!("Failed to derive experiment checkpoint image name for {requested_image_name}")
        })
    }

    /// List every SQLite ref whose `(name, reference)` matches the
    /// shape an anonymous artifact's image name would take:
    /// `<registry-id8>.ommx.local/anonymous` (8 lowercase hex chars
    /// prefix + suffix) for the name, and `YYYYMMDDTHHMMSS-<nonce>`
    /// (timestamp + 12-hex random suffix) for the reference. Both
    /// must match — a substring check on the suffix alone would
    /// over-match a human-pushed ref against a real mDNS host like
    /// `myhost.ommx.local/anonymous:v1`. Returned in
    /// `(name, reference)` order to match the SQLite index order.
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
    /// `ommx prune-anonymous`) can report what changed. The
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

    /// List every image ref stored in this registry.
    pub fn list_image_refs(&self) -> Result<Vec<ImageRef>> {
        self.index
            .list_refs(None)?
            .into_iter()
            .map(|r| ImageRef::from_repository_and_reference(&r.name, &r.reference))
            .collect()
    }

    /// List Experiment refs stored in this registry, optionally filtered by
    /// full image-reference prefix.
    pub fn list_experiments(&self, name_prefix: Option<&str>) -> Result<Vec<ExperimentRefRecord>> {
        let refs = self.index.list_refs(name_prefix)?;
        let cached = self.index.list_experiment_refs(name_prefix)?;
        let cached_refs = cached
            .iter()
            .map(|r| r.image_name.to_string())
            .collect::<BTreeSet<_>>();
        for r in refs {
            let image_name = ImageRef::from_repository_and_reference(&r.name, &r.reference)?;
            if cached_refs.contains(&image_name.to_string()) {
                continue;
            }
            self.backfill_experiment_manifest(&image_name, r.descriptor.digest())?;
        }
        self.index.list_experiment_refs(name_prefix)
    }

    fn backfill_experiment_manifest(
        &self,
        image_name: &ImageRef,
        manifest_digest: &Digest,
    ) -> Result<()> {
        if let Some(record) = self.experiment_manifest_record(image_name, manifest_digest)? {
            self.index.upsert_experiment_manifest(&record)?;
        }
        Ok(())
    }

    /// Build a validated Experiment listing projection for a stored manifest.
    ///
    /// Returns `None` for non-Experiment artifacts.
    pub(crate) fn experiment_manifest_record(
        &self,
        image_name: &ImageRef,
        manifest_digest: &Digest,
    ) -> Result<Option<ExperimentManifestRecord>> {
        let artifact = LocalArtifact::from_parts(self, image_name.clone(), manifest_digest.clone());
        crate::experiment::experiment_manifest_record_from_artifact(&artifact)
    }

    /// Seal an unsealed OMMX Artifact manifest into the Local Registry.
    ///
    /// The manifest's config/layers are represented as
    /// [`StoredDescriptor`] before this method is called, so sealing
    /// does not re-validate dependency blob existence. It serializes
    /// and stores only the root manifest blob, yielding its root
    /// [`SealedArtifact`].
    /// Crate-visible because artifact builders live in sibling modules; the
    /// sealing invariant and manifest byte ownership stay inside the registry.
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
    /// blobs or manifest bytes. Crate-visible for sibling artifact /
    /// experiment commit paths that already hold a [`SealedArtifact`].
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

    /// Publish a sealed Experiment manifest and its verified listing projection
    /// under an image ref in one SQLite transaction.
    pub(crate) fn publish_experiment_manifest_ref(
        &self,
        image_name: &ImageRef,
        sealed_artifact: &SealedArtifact<'_>,
        experiment: &ExperimentManifestRecord,
    ) -> Result<RefUpdate> {
        ensure!(
            sealed_artifact.is_stored_in(self),
            "Sealed artifact descriptor belongs to a different Local Registry"
        );
        self.index
            .publish_experiment_ref(image_name, &sealed_artifact.0, experiment)
    }

    /// Publish an already-stored root manifest descriptor under an image ref.
    ///
    /// This is used when adding another local name for an existing artifact.
    /// It is an IndexStore operation only: no payload blobs or manifest bytes
    /// are rewritten. Crate-visible for `LocalArtifact::tag_as`.
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
    /// blobs or manifest bytes. Crate-visible for sibling artifact /
    /// experiment commit paths that already hold a [`SealedArtifact`].
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

    /// Replace an Experiment ref and its verified listing projection in one
    /// SQLite transaction.
    pub(crate) fn replace_experiment_manifest_ref(
        &self,
        image_name: &ImageRef,
        sealed_artifact: &SealedArtifact<'_>,
        experiment: &ExperimentManifestRecord,
    ) -> Result<RefUpdate> {
        ensure!(
            sealed_artifact.is_stored_in(self),
            "Sealed artifact descriptor belongs to a different Local Registry"
        );
        self.index
            .replace_experiment_ref(image_name, &sealed_artifact.0, experiment)
    }

    /// Delete a local manifest ref. Content-addressed blobs are not removed.
    /// Crate-visible for Experiment checkpoint cleanup; GC handles blob removal.
    pub(crate) fn delete_manifest_ref(&self, image_name: &ImageRef) -> Result<bool> {
        self.index
            .delete_ref(&image_name.repository_key(), image_name.reference())
    }

    fn store_blob_bytes(&self, bytes: &[u8]) -> Result<Digest> {
        self.blobs.put_bytes(bytes)
    }

    /// Read a raw blob by digest for crate-internal artifact materialization.
    ///
    /// Public callers read through [`LocalRegistry::get_blob`] with a
    /// [`StoredDescriptor`]. This digest-only form is crate-visible for
    /// manifest parsing, save/push, and GC traversal across top-level modules.
    pub(crate) fn read_blob(&self, digest: &Digest) -> Result<Vec<u8>> {
        self.blobs.read_bytes(digest)
    }

    /// Check raw blob presence for crate-internal import / test guards.
    ///
    /// This remains crate-visible because remote pull and artifact tests need to
    /// distinguish a present SQLite ref from a missing manifest blob.
    pub(crate) fn contains_blob(&self, digest: &Digest) -> Result<bool> {
        self.blobs.exists(digest)
    }

    /// Read raw blob size when promoting an OCI descriptor to a registry-owned
    /// [`StoredDescriptor`] across top-level artifact / experiment modules.
    pub(crate) fn blob_size(&self, digest: &Digest) -> Result<u64> {
        self.blobs.size(digest)
    }

    /// Touch a raw blob's mtime for crate-internal ref-preservation flows.
    ///
    /// The public API does not expose mtime management; this is used when
    /// registry-owned manifest closures are re-tagged.
    pub(crate) fn touch_blob(&self, digest: &Digest) -> Result<()> {
        self.blobs.touch_blob(digest)
    }

    fn list_blob_records(&self) -> Result<Vec<BlobRecord>> {
        self.blobs.list_blobs()
    }

    fn delete_blob_if_older_than(
        &self,
        digest: &Digest,
        cutoff: std::time::SystemTime,
    ) -> Result<DeleteBlobOutcome> {
        self.blobs.delete_blob_if_older_than(digest, cutoff)
    }

    /// Build a registry-owned manifest descriptor from a stored manifest digest.
    ///
    /// Crate-visible for `LocalArtifact` handles and Experiment subject links.
    pub(crate) fn stored_manifest_descriptor(
        &self,
        manifest_digest: &Digest,
    ) -> Result<StoredDescriptor<'_>> {
        let size = self.blob_size(manifest_digest)?;
        let descriptor = DescriptorBuilder::default()
            .media_type(MediaType::ImageManifest)
            .digest(manifest_digest.clone())
            .size(size)
            .build()
            .context("Failed to build manifest descriptor")?;
        self.stored_descriptor(descriptor)
    }

    /// Touch every blob reachable from a manifest, including subject manifests.
    ///
    /// Crate-visible for `LocalArtifact::tag_as`; the traversal is registry
    /// internal so callers do not pass BLOB protection lists around.
    pub(crate) fn touch_manifest_closure(
        &self,
        manifest_digest: &Digest,
        visited: &mut BTreeSet<String>,
    ) -> Result<()> {
        if !visited.insert(manifest_digest.as_ref().to_string()) {
            return Ok(());
        }
        self.touch_blob(manifest_digest)?;
        let bytes = self
            .read_blob(manifest_digest)
            .with_context(|| format!("Failed to read manifest blob {manifest_digest}"))?;
        let manifest: ImageManifest = serde_json::from_slice(&bytes)
            .with_context(|| format!("Failed to parse OCI image manifest {manifest_digest}"))?;

        self.touch_descriptor_blob(manifest.config())?;
        for layer in manifest.layers() {
            self.touch_descriptor_blob(layer)?;
        }
        if let Some(subject) = manifest.subject() {
            let subject = self.stored_descriptor(subject.clone())?;
            self.touch_manifest_closure(subject.digest(), visited)?;
        }
        Ok(())
    }

    fn touch_descriptor_blob(&self, descriptor: &Descriptor) -> Result<()> {
        let descriptor = self.stored_descriptor(descriptor.clone())?;
        self.touch_blob(descriptor.digest())
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
    /// content-addressed blob. The descriptor carries the supplied media type
    /// and annotations, and its digest / size are derived from `bytes`.
    /// Crate-visible for Experiment and artifact attachment builders that live
    /// outside the `local_registry` module.
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
    /// this registry. Crate-visible for Experiment trace/checkpoint layers.
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
    /// descriptor without layer annotations. Crate-visible for Experiment
    /// config blobs created outside the `local_registry` module.
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
    /// verify the concrete bytes match the descriptor. Crate-visible for
    /// artifact builders and imports that must return a [`StoredDescriptor`]
    /// while keeping the raw filesystem CAS private.
    pub(crate) fn store_blob(
        &self,
        descriptor: Descriptor,
        bytes: &[u8],
    ) -> Result<StoredDescriptor<'_>> {
        let digest = self.store_blob_bytes(bytes)?;
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
    /// registry and promote it to a [`StoredDescriptor`]. Crate-visible for
    /// `LocalArtifact` and Experiment recovery paths that store descriptors
    /// durably and revalidate them on use.
    pub(crate) fn stored_descriptor(&self, descriptor: Descriptor) -> Result<StoredDescriptor<'_>> {
        let size = self.blob_size(descriptor.digest())?;
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
