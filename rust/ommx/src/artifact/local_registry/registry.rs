use super::index::{CachedRefIdentity, CachedRefRead as IndexCachedRefRead, SqliteIndexStore};
use super::{
    ArtifactListOptions, ArtifactManifestRecord, ArtifactRefRecord,
    ExperimentCheckpointListOptions, ExperimentCheckpointRefRecord, ExperimentListOptions,
    ExperimentManifestRecord, ExperimentRefRecord, RefUpdate, RegistryListReport,
    RegistryListWarning, RegistryListWarningStage,
};
use crate::artifact::{
    media_types::{self, RootPayloadVersion},
    sha256_digest, stable_json_bytes, ImageRef, LocalArtifact,
};
use anyhow::{bail, ensure, Context, Result};
use oci_spec::image::{Descriptor, DescriptorBuilder, Digest, ImageManifest, MediaType};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::OnceLock;

pub mod attachment_storage;
mod blob;
mod gc;
mod import;

#[cfg(feature = "remote-artifact")]
pub mod async_io {
    use super::LocalRegistry;
    use anyhow::{ensure, Result};
    use oci_spec::image::Descriptor;

    /// Read and verify one descriptor's blob without blocking the async OCI
    /// transport runtime. Filesystem I/O and digest verification run on
    /// Tokio's blocking pool because the Local Registry blob store is
    /// intentionally synchronous outside remote transfer pipelines.
    pub async fn read_descriptor_blob(
        registry: &LocalRegistry,
        descriptor: &Descriptor,
    ) -> Result<Vec<u8>> {
        let bytes = registry.blobs.read_bytes_async(descriptor.digest()).await?;
        ensure!(
            bytes.len() as u64 == descriptor.size(),
            "Descriptor size mismatch for {}: descriptor={}, actual={}",
            descriptor.digest(),
            descriptor.size(),
            bytes.len()
        );
        Ok(bytes)
    }

    /// Store and verify one descriptor's blob without blocking the async OCI
    /// transport runtime. The synchronous CAS publication remains owned by
    /// `FileBlobStore`, but runs on Tokio's blocking pool.
    pub async fn store_descriptor_blob(
        registry: &LocalRegistry,
        descriptor: Descriptor,
        bytes: Vec<u8>,
    ) -> Result<()> {
        let actual_size = bytes.len() as u64;
        let digest = registry.blobs.put_bytes_async(bytes).await?;
        ensure!(
            &digest == descriptor.digest(),
            "Descriptor digest mismatch: descriptor={}, actual={}",
            descriptor.digest(),
            digest
        );
        ensure!(
            actual_size == descriptor.size(),
            "Descriptor size mismatch for {}: descriptor={}, actual={}",
            descriptor.digest(),
            descriptor.size(),
            actual_size
        );
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::artifact::sha256_digest;
        use oci_spec::image::{DescriptorBuilder, Digest, MediaType};
        use std::str::FromStr;

        #[test]
        fn reads_and_verifies_blob_on_blocking_pool() {
            let temp = crate::artifact::local_registry::TempLocalRegistry::new().unwrap();
            let bytes = b"async local blob";
            let descriptor = DescriptorBuilder::default()
                .media_type(MediaType::Other("application/octet-stream".to_string()))
                .digest(Digest::from_str(&sha256_digest(bytes)).unwrap())
                .size(bytes.len() as u64)
                .build()
                .unwrap();
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap();

            let actual = runtime.block_on(async {
                store_descriptor_blob(temp.registry(), descriptor.clone(), bytes.to_vec()).await?;
                read_descriptor_blob(temp.registry(), &descriptor).await
            });

            assert_eq!(actual.unwrap(), bytes);
        }
    }
}

use blob::{BlobRecord, DeleteBlobOutcome, FileBlobStore};
pub use gc::{
    GcBlob, GcDeleteReport, GcInvalidManifest, GcMissingBlob, GcOptions, GcReferenceKind, GcReport,
    GcRoot,
};
pub use import::{ArchiveInspectView, LegacyImportReport, OciDirImport, OciDirRef};

static DEFAULT_LOCAL_REGISTRY: OnceLock<LocalRegistry> = OnceLock::new();
const EXPERIMENT_CHECKPOINT_REPOSITORY: &str = "checkpoint";
const FILE_BLOB_STORE_DIR_NAME: &str = "blobs";
const GC_EXCLUSION_LOCK_FILE_NAME: &str = ".gc-exclusion.lock";

/// OMMX-owned signal that a persisted Local Registry ref cannot be
/// reconstructed as an [`ImageRef`].
///
/// [`LocalRegistry::list_image_refs`] and [`crate::artifact::get_images`] keep
/// returning [`crate::Result`]. Callers can downcast the returned error to this
/// type to distinguish corrupted registry state from invalid user input. The
/// source chain retains the underlying [`crate::artifact::ImageRefParseError`].
#[derive(Debug, thiserror::Error)]
#[error(
    "Invalid Local Registry image ref with name {name:?} and reference {reference:?}: {source}"
)]
pub struct InvalidLocalRegistryImageRef {
    name: String,
    reference: String,
    #[source]
    source: crate::Error,
}

impl InvalidLocalRegistryImageRef {
    /// Persisted repository name that could not be reconstructed.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Persisted tag or digest reference that could not be reconstructed.
    pub fn reference(&self) -> &str {
        &self.reference
    }
}

struct InvalidCachedRefs {
    manifest_digest: Digest,
    refs: Vec<(ImageRef, String)>,
}

struct CachedRefRead<T> {
    image_name: ImageRef,
    manifest_digest: Digest,
    record: std::result::Result<T, String>,
}

struct RefGroup {
    manifest_digest: Digest,
    refs: Vec<ImageRef>,
}

fn group_refs_by_manifest_digest(
    refs: impl IntoIterator<Item = (ImageRef, Digest)>,
) -> BTreeMap<String, RefGroup> {
    let mut groups = BTreeMap::new();
    for (image_name, manifest_digest) in refs {
        groups
            .entry(manifest_digest.to_string())
            .or_insert_with(|| RefGroup {
                manifest_digest,
                refs: Vec::new(),
            })
            .refs
            .push(image_name);
    }
    groups
}

fn invalid_cached_refs<T>(reads: &[CachedRefRead<T>]) -> BTreeMap<String, InvalidCachedRefs> {
    let mut groups = BTreeMap::new();
    for read in reads {
        let Err(error) = &read.record else {
            continue;
        };
        groups
            .entry(read.manifest_digest.to_string())
            .or_insert_with(|| InvalidCachedRefs {
                manifest_digest: read.manifest_digest.clone(),
                refs: Vec::new(),
            })
            .refs
            .push((read.image_name.clone(), error.clone()));
    }
    groups
}

fn validate_ref_identities(
    identities: Vec<CachedRefIdentity>,
    include_repository: &impl Fn(&str) -> bool,
    strict: bool,
    warnings: &mut Vec<RegistryListWarning>,
    stage: RegistryListWarningStage,
) -> Result<Vec<(ImageRef, Digest)>> {
    let mut valid = Vec::new();
    for identity in identities {
        if !include_repository(&identity.name) {
            continue;
        }
        let raw_image_name = identity.image_name();
        let raw_manifest_digest = identity.manifest_digest.clone();
        match identity.parsed {
            Ok(parsed) => valid.push(parsed),
            Err(error) => push_warning_or_error(
                strict,
                warnings,
                registry_list_warning(raw_image_name, raw_manifest_digest, stage, error),
            )?,
        }
    }
    Ok(valid)
}

fn validate_cached_ref_reads<T>(
    reads: Vec<IndexCachedRefRead<T>>,
    include_repository: &impl Fn(&str) -> bool,
    strict: bool,
    warnings: &mut Vec<RegistryListWarning>,
    stage: RegistryListWarningStage,
) -> Result<Vec<CachedRefRead<T>>> {
    let mut valid = Vec::new();
    for read in reads {
        if !include_repository(&read.identity.name) {
            continue;
        }
        let raw_image_name = read.identity.image_name();
        let raw_manifest_digest = read.identity.manifest_digest.clone();
        match read.identity.parsed {
            Ok((image_name, manifest_digest)) => valid.push(CachedRefRead {
                image_name,
                manifest_digest,
                record: read.record,
            }),
            Err(error) => push_warning_or_error(
                strict,
                warnings,
                registry_list_warning(raw_image_name, raw_manifest_digest, stage, error),
            )?,
        }
    }
    Ok(valid)
}

fn registry_list_warning(
    image_name: impl ToString,
    manifest_digest: impl ToString,
    stage: RegistryListWarningStage,
    message: String,
) -> RegistryListWarning {
    RegistryListWarning {
        image_name: image_name.to_string(),
        manifest_digest: manifest_digest.to_string(),
        stage,
        message,
    }
}

fn push_warning_or_error(
    strict: bool,
    warnings: &mut Vec<RegistryListWarning>,
    warning: RegistryListWarning,
) -> Result<()> {
    if strict {
        bail!("{warning}");
    }
    if !warnings.contains(&warning) {
        warnings.push(warning);
    }
    Ok(())
}

fn log_registry_list_warnings(warnings: &[RegistryListWarning]) {
    for warning in warnings {
        tracing::warn!(
            image_name = %warning.image_name,
            manifest_digest = %warning.manifest_digest,
            stage = %warning.stage,
            error = %warning.message,
            "Local Registry listing skipped or repaired a ref"
        );
    }
}

fn experiment_checkpoint_ref_from_record(
    registry: &LocalRegistry,
    record: ExperimentRefRecord,
) -> Result<ExperimentCheckpointRefRecord> {
    let requested_image_name = record
        .config
        .get("requested_image_name")
        .and_then(serde_json::Value::as_str)
        .context("Experiment checkpoint Config is missing `requested_image_name`")?;
    let requested_image_name = ImageRef::parse(requested_image_name)
        .context("Experiment checkpoint Config contains an invalid `requested_image_name`")?;
    let status = crate::experiment::ExperimentStatus::from_config(&record.status)?;
    ensure!(
        status != crate::experiment::ExperimentStatus::Finished,
        "Internal Experiment checkpoint has `finished` status"
    );
    let expected_checkpoint_image_name =
        registry.experiment_checkpoint_image_name(&requested_image_name)?;
    ensure!(
        record.image_name == expected_checkpoint_image_name,
        "Experiment checkpoint ref does not match `requested_image_name`: expected {}, got {}",
        expected_checkpoint_image_name,
        record.image_name,
    );
    Ok(ExperimentCheckpointRefRecord {
        checkpoint_image_name: record.image_name,
        requested_image_name,
        manifest_digest: record.manifest_digest,
        config_digest: record.config_digest,
        updated_at: record.updated_at,
        status: record.status,
        run_count: record.run_count,
        solve_count: record.solve_count,
        sampling_count: record.sampling_count,
        annotations: record.annotations,
        config: record.config,
    })
}

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

/// An existing manifest whose complete config/layer/subject closure has been
/// verified from this Local Registry's CAS.
///
/// This capability stays private to the registry owner so an existing digest
/// cannot be published without first proving that it remains materializable.
struct VerifiedManifestClosure<'reg> {
    manifest: StoredDescriptor<'reg>,
    projection: VerifiedManifestProjection,
}

enum VerifiedManifestProjection {
    Artifact(ArtifactManifestRecord),
    Experiment(ExperimentManifestRecord),
}

struct GcExclusionGuard {
    file: File,
}

impl Drop for GcExclusionGuard {
    fn drop(&mut self) {
        let _ = self.file.unlock();
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
        let repository_key = self.experiment_checkpoint_repository_key()?;
        let digest = sha256_digest(requested_image_name.to_string().as_bytes());
        let tag = digest
            .strip_prefix("sha256:")
            .expect("sha256_digest returns a sha256-prefixed digest");
        ImageRef::parse(&format!("{repository_key}:{tag}")).with_context(|| {
            format!("Failed to derive experiment checkpoint image name for {requested_image_name}")
        })
    }

    fn experiment_checkpoint_repository_key(&self) -> Result<String> {
        let registry_id = self.index.registry_id()?;
        crate::artifact::anonymous_local_repository_key(
            &registry_id,
            EXPERIMENT_CHECKPOINT_REPOSITORY,
        )
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
        self.list_anonymous_refs(&crate::artifact::local_registry::AnonymousRefOptions::default())
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
        self.prune_anonymous_refs(&crate::artifact::local_registry::AnonymousRefOptions::default())
    }

    /// List synthetic anonymous refs eligible for cleanup.
    ///
    /// Anonymous Artifact refs are always included. Options can additionally
    /// include anonymous Experiment refs and restrict results by ref age.
    pub fn list_anonymous_refs(
        &self,
        options: &crate::artifact::local_registry::AnonymousRefOptions,
    ) -> Result<Vec<crate::artifact::local_registry::RefRecord>> {
        let cutoff = options
            .older_than
            .map(|age| {
                let age = chrono::Duration::from_std(age)
                    .context("Anonymous ref retention duration is too large")?;
                chrono::Utc::now()
                    .checked_sub_signed(age)
                    .context("Anonymous ref retention cutoff is out of range")
            })
            .transpose()?;
        let mut refs = Vec::new();
        for record in self.index.list_refs(None)? {
            let anonymous_artifact = crate::artifact::is_anonymous_artifact_ref_name(&record.name);
            let anonymous_experiment = options.include_experiments
                && crate::artifact::is_anonymous_experiment_ref_name(&record.name);
            if !(anonymous_artifact || anonymous_experiment)
                || !crate::artifact::is_anonymous_artifact_tag(&record.reference)
            {
                continue;
            }
            if let Some(cutoff) = cutoff {
                let updated_at = chrono::DateTime::parse_from_rfc3339(&record.updated_at)
                    .with_context(|| {
                        format!(
                            "Invalid Local Registry ref timestamp for {}:{}",
                            record.name, record.reference
                        )
                    })?
                    .with_timezone(&chrono::Utc);
                if updated_at > cutoff {
                    continue;
                }
            }
            refs.push(record);
        }
        Ok(refs)
    }

    /// Delete synthetic anonymous refs selected by [`Self::list_anonymous_refs`].
    ///
    /// Only SQLite refs are removed. Immutable manifest and payload blobs stay
    /// in the CAS until [`Self::gc`] reclaims them. Candidate rows are deleted
    /// only if they have not changed since selection.
    pub fn prune_anonymous_refs(
        &self,
        options: &crate::artifact::local_registry::AnonymousRefOptions,
    ) -> Result<Vec<crate::artifact::local_registry::RefRecord>> {
        let refs = self.list_anonymous_refs(options)?;
        self.index.delete_refs_if_unchanged(&refs)
    }

    /// List every image ref stored in this registry.
    ///
    /// A persisted ref that cannot be reconstructed retains an
    /// [`InvalidLocalRegistryImageRef`] signal in the returned error.
    pub fn list_image_refs(&self) -> Result<Vec<ImageRef>> {
        self.index
            .list_refs(None)?
            .into_iter()
            .map(|record| {
                let name = record.name;
                let reference = record.reference;
                ImageRef::from_repository_and_reference(&name, &reference).map_err(|source| {
                    crate::error!(InvalidLocalRegistryImageRef {
                        name,
                        reference,
                        source,
                    })
                })
            })
            .collect()
    }

    /// List OMMX Artifact refs stored in this registry, optionally filtered by
    /// full image-reference prefix.
    ///
    /// Missing Manifest cache rows are backfilled from the content-addressed
    /// blob store before the digest-validated SQLite projections are returned.
    /// Internal implementation refs, including Experiment checkpoints, are
    /// excluded from this user-facing catalog.
    pub fn list_artifacts(&self, name_prefix: Option<&str>) -> Result<Vec<ArtifactRefRecord>> {
        let report =
            self.list_artifacts_with_options(name_prefix, &ArtifactListOptions::default())?;
        log_registry_list_warnings(&report.warnings);
        Ok(report.records)
    }

    /// List OMMX Artifact refs with explicit internal-ref and corruption
    /// handling options.
    pub fn list_artifacts_with_options(
        &self,
        name_prefix: Option<&str>,
        options: &ArtifactListOptions,
    ) -> Result<RegistryListReport<ArtifactRefRecord>> {
        let checkpoint_repository_key = self.experiment_checkpoint_repository_key()?;
        let include_repository = |repository_key: &str| {
            options.include_internal || repository_key != checkpoint_repository_key
        };
        let mut warnings =
            self.backfill_artifact_manifests(name_prefix, options.strict, &include_repository)?;
        let mut reads = validate_cached_ref_reads(
            self.index.list_artifact_ref_reads(name_prefix)?,
            &include_repository,
            options.strict,
            &mut warnings,
            RegistryListWarningStage::ManifestCacheRepair,
        )?;

        let invalid = invalid_cached_refs(&reads);
        if options.strict {
            if let Some(group) = invalid.values().next() {
                let (image_name, message) = &group.refs[0];
                bail!(
                    "{}",
                    registry_list_warning(
                        image_name.clone(),
                        group.manifest_digest.clone(),
                        RegistryListWarningStage::ManifestCacheRepair,
                        message.clone(),
                    )
                );
            }
        }

        let mut repair_failures = BTreeMap::new();
        for (digest_key, group) in &invalid {
            match self.artifact_manifest_record(&group.manifest_digest) {
                Ok(record) => {
                    self.index.upsert_artifact_manifest(&record)?;
                    for (image_name, message) in &group.refs {
                        warnings.push(registry_list_warning(
                            image_name.clone(),
                            group.manifest_digest.clone(),
                            RegistryListWarningStage::ManifestCacheRepair,
                            format!("Invalid cached Manifest was repaired from CAS: {message}"),
                        ));
                    }
                }
                Err(error) => {
                    repair_failures.insert(digest_key.clone(), format!("{error:#}"));
                }
            }
        }

        if !invalid.is_empty() {
            reads = validate_cached_ref_reads(
                self.index.list_artifact_ref_reads(name_prefix)?,
                &include_repository,
                options.strict,
                &mut warnings,
                RegistryListWarningStage::ManifestCacheRepair,
            )?;
        }

        let mut records = Vec::new();
        for read in reads {
            match read.record {
                Ok(record) => records.push(record),
                Err(error) => {
                    let repair_error = repair_failures
                        .get(read.manifest_digest.as_ref())
                        .map(|repair_error| format!("; CAS repair failed: {repair_error}"))
                        .unwrap_or_default();
                    warnings.push(registry_list_warning(
                        read.image_name,
                        read.manifest_digest,
                        RegistryListWarningStage::ManifestCacheRepair,
                        format!("Invalid cached Manifest: {error}{repair_error}"),
                    ));
                }
            }
        }
        Ok(RegistryListReport { records, warnings })
    }

    /// List Experiment refs stored in this registry, optionally filtered by
    /// full image-reference prefix.
    pub fn list_experiments(&self, name_prefix: Option<&str>) -> Result<Vec<ExperimentRefRecord>> {
        let report =
            self.list_experiments_with_options(name_prefix, &ExperimentListOptions::default())?;
        log_registry_list_warnings(&report.warnings);
        Ok(report.records)
    }

    /// List committed Experiment refs with explicit corruption handling.
    pub fn list_experiments_with_options(
        &self,
        name_prefix: Option<&str>,
        options: &ExperimentListOptions,
    ) -> Result<RegistryListReport<ExperimentRefRecord>> {
        let checkpoint_repository_key = self.experiment_checkpoint_repository_key()?;
        let artifact_report = self.list_artifacts_with_options(
            name_prefix,
            &ArtifactListOptions {
                include_internal: false,
                strict: options.strict,
            },
        )?;
        let mut report =
            self.list_experiment_records(name_prefix, options.strict, |repository_key| {
                repository_key != checkpoint_repository_key
            })?;
        report.warnings.splice(0..0, artifact_report.warnings);
        Ok(report)
    }

    /// List recoverable Experiment checkpoints by requested image-name prefix.
    pub fn list_experiment_checkpoints(
        &self,
        requested_name_prefix: Option<&str>,
    ) -> Result<Vec<ExperimentCheckpointRefRecord>> {
        let report = self.list_experiment_checkpoints_with_options(
            requested_name_prefix,
            &ExperimentCheckpointListOptions::default(),
        )?;
        log_registry_list_warnings(&report.warnings);
        Ok(report.records)
    }

    /// List recoverable Experiment checkpoints with lifecycle-status and
    /// corruption handling options.
    pub fn list_experiment_checkpoints_with_options(
        &self,
        requested_name_prefix: Option<&str>,
        options: &ExperimentCheckpointListOptions,
    ) -> Result<RegistryListReport<ExperimentCheckpointRefRecord>> {
        ensure!(
            options
                .statuses
                .iter()
                .all(|status| *status != crate::experiment::ExperimentStatus::Finished),
            "Experiment checkpoint status filter cannot include `finished`"
        );
        let checkpoint_repository_key = self.experiment_checkpoint_repository_key()?;
        let checkpoint_prefix = format!("{checkpoint_repository_key}:");
        let artifact_report = self.list_artifacts_with_options(
            Some(&checkpoint_prefix),
            &ArtifactListOptions {
                include_internal: true,
                strict: options.strict,
            },
        )?;
        let experiment_report = self.list_experiment_records(
            Some(&checkpoint_prefix),
            options.strict,
            |repository_key| repository_key == checkpoint_repository_key,
        )?;
        let mut warnings = artifact_report.warnings;
        warnings.extend(experiment_report.warnings);
        let mut records = Vec::new();
        for record in experiment_report.records {
            let checkpoint_image_name = record.image_name.clone();
            let manifest_digest = record.manifest_digest.clone();
            match experiment_checkpoint_ref_from_record(self, record) {
                Ok(record) => {
                    let prefix_matches = requested_name_prefix
                        .map(|prefix| record.requested_image_name.to_string().starts_with(prefix))
                        .unwrap_or(true);
                    let status_matches = options.statuses.is_empty()
                        || options
                            .statuses
                            .iter()
                            .any(|status| status.as_str() == record.status);
                    if prefix_matches && status_matches {
                        records.push(record);
                    }
                }
                Err(error) => {
                    let warning = registry_list_warning(
                        checkpoint_image_name,
                        manifest_digest,
                        RegistryListWarningStage::CheckpointProjection,
                        format!("{error:#}"),
                    );
                    push_warning_or_error(options.strict, &mut warnings, warning)?;
                }
            }
        }
        records.sort_by(|left, right| {
            left.requested_image_name
                .to_string()
                .cmp(&right.requested_image_name.to_string())
                .then_with(|| {
                    left.checkpoint_image_name
                        .to_string()
                        .cmp(&right.checkpoint_image_name.to_string())
                })
        });
        Ok(RegistryListReport { records, warnings })
    }

    fn list_experiment_records(
        &self,
        name_prefix: Option<&str>,
        strict: bool,
        include_repository: impl Fn(&str) -> bool,
    ) -> Result<RegistryListReport<ExperimentRefRecord>> {
        let mut warnings = Vec::new();
        let missing = validate_ref_identities(
            self.index
                .list_missing_experiment_config_refs(name_prefix)?,
            &include_repository,
            strict,
            &mut warnings,
            RegistryListWarningStage::ExperimentConfigBackfill,
        )?;
        let groups = group_refs_by_manifest_digest(missing);
        for group in groups.values() {
            let image_name = group.refs[0].clone();
            match self.required_experiment_manifest_record(&image_name, &group.manifest_digest) {
                Ok(record) => self.index.upsert_experiment_manifest(&record)?,
                Err(error) => {
                    for image_name in &group.refs {
                        let warning = registry_list_warning(
                            image_name.clone(),
                            group.manifest_digest.clone(),
                            RegistryListWarningStage::ExperimentConfigBackfill,
                            format!("{error:#}"),
                        );
                        push_warning_or_error(strict, &mut warnings, warning)?;
                    }
                }
            }
        }

        let mut reads = validate_cached_ref_reads(
            self.index.list_experiment_ref_reads(name_prefix)?,
            &include_repository,
            strict,
            &mut warnings,
            RegistryListWarningStage::ExperimentConfigCacheRepair,
        )?;
        let invalid = invalid_cached_refs(&reads);
        if strict {
            if let Some(group) = invalid.values().next() {
                let (image_name, message) = &group.refs[0];
                bail!(
                    "{}",
                    registry_list_warning(
                        image_name.clone(),
                        group.manifest_digest.clone(),
                        RegistryListWarningStage::ExperimentConfigCacheRepair,
                        message.clone(),
                    )
                );
            }
        }

        let mut repair_failures = BTreeMap::new();
        for (digest_key, group) in &invalid {
            let image_name = group.refs[0].0.clone();
            match self.required_experiment_manifest_record(&image_name, &group.manifest_digest) {
                Ok(record) => {
                    self.index.upsert_experiment_manifest(&record)?;
                    for (image_name, message) in &group.refs {
                        warnings.push(registry_list_warning(
                            image_name.clone(),
                            group.manifest_digest.clone(),
                            RegistryListWarningStage::ExperimentConfigCacheRepair,
                            format!(
                                "Invalid cached Experiment projection was repaired from CAS: {message}"
                            ),
                        ));
                    }
                }
                Err(error) => {
                    repair_failures.insert(digest_key.clone(), format!("{error:#}"));
                }
            }
        }
        if !invalid.is_empty() {
            reads = validate_cached_ref_reads(
                self.index.list_experiment_ref_reads(name_prefix)?,
                &include_repository,
                strict,
                &mut warnings,
                RegistryListWarningStage::ExperimentConfigCacheRepair,
            )?;
        }

        let mut records = Vec::new();
        for read in reads {
            match read.record {
                Ok(record) => records.push(record),
                Err(error) => {
                    let repair_error = repair_failures
                        .get(read.manifest_digest.as_ref())
                        .map(|repair_error| format!("; CAS repair failed: {repair_error}"))
                        .unwrap_or_default();
                    warnings.push(registry_list_warning(
                        read.image_name,
                        read.manifest_digest,
                        RegistryListWarningStage::ExperimentConfigCacheRepair,
                        format!("Invalid cached Experiment projection: {error}{repair_error}"),
                    ));
                }
            }
        }
        Ok(RegistryListReport { records, warnings })
    }

    fn backfill_artifact_manifests(
        &self,
        name_prefix: Option<&str>,
        strict: bool,
        include_repository: &impl Fn(&str) -> bool,
    ) -> Result<Vec<RegistryListWarning>> {
        let mut warnings = Vec::new();
        let missing = validate_ref_identities(
            self.index
                .list_missing_artifact_manifest_refs(name_prefix)?,
            include_repository,
            strict,
            &mut warnings,
            RegistryListWarningStage::ManifestBackfill,
        )?;
        let groups = group_refs_by_manifest_digest(missing);
        for group in groups.values() {
            match self.artifact_manifest_record(&group.manifest_digest) {
                Ok(record) => self.index.upsert_artifact_manifest(&record)?,
                Err(error) => {
                    for image_name in &group.refs {
                        let warning = registry_list_warning(
                            image_name.clone(),
                            group.manifest_digest.clone(),
                            RegistryListWarningStage::ManifestBackfill,
                            format!("{error:#}"),
                        );
                        push_warning_or_error(strict, &mut warnings, warning)?;
                    }
                }
            }
        }
        Ok(warnings)
    }

    fn required_experiment_manifest_record(
        &self,
        image_name: &ImageRef,
        manifest_digest: &Digest,
    ) -> Result<ExperimentManifestRecord> {
        self.experiment_manifest_record(image_name, manifest_digest)?
            .context("Manifest is not an Experiment Artifact")
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

    pub(crate) fn artifact_manifest_record(
        &self,
        manifest_digest: &Digest,
    ) -> Result<ArtifactManifestRecord> {
        let (manifest_json, manifest, _) = self.read_artifact_manifest(manifest_digest)?;
        ArtifactManifestRecord::from_image_manifest(
            manifest_digest.clone(),
            manifest_json,
            &manifest,
        )
    }

    fn read_artifact_manifest(
        &self,
        manifest_digest: &Digest,
    ) -> Result<(Vec<u8>, ImageManifest, Descriptor)> {
        let manifest_json = self.read_blob(manifest_digest)?;
        let manifest: ImageManifest = serde_json::from_slice(&manifest_json)
            .with_context(|| format!("Failed to parse OCI image manifest {manifest_digest}"))?;
        Self::validate_manifest(&manifest)?;
        let manifest_descriptor = Self::build_manifest_descriptor(&manifest_json)?;
        ensure!(
            manifest_descriptor.digest() == manifest_digest,
            "Manifest blob digest mismatch: requested {}, computed {}",
            manifest_digest,
            manifest_descriptor.digest()
        );
        Ok((manifest_json, manifest, manifest_descriptor))
    }

    fn verify_manifest_closure(
        &self,
        image_name: &ImageRef,
        manifest_digest: &Digest,
    ) -> Result<VerifiedManifestClosure<'_>> {
        let (manifest_json, manifest, manifest_descriptor) =
            self.read_artifact_manifest(manifest_digest)?;
        let mut visited = BTreeSet::from([manifest_digest.as_ref().to_string()]);
        self.verify_manifest_dependencies(&manifest, &mut visited)?;
        let artifact = ArtifactManifestRecord::from_image_manifest(
            manifest_digest.clone(),
            manifest_json,
            &manifest,
        )?;
        let projection = match self.experiment_manifest_record(image_name, manifest_digest)? {
            Some(experiment) => VerifiedManifestProjection::Experiment(experiment),
            None => VerifiedManifestProjection::Artifact(artifact),
        };
        let manifest = self.stored_descriptor(manifest_descriptor)?;
        Ok(VerifiedManifestClosure {
            manifest,
            projection,
        })
    }

    fn verify_manifest_dependencies(
        &self,
        manifest: &ImageManifest,
        visited: &mut BTreeSet<String>,
    ) -> Result<()> {
        self.verify_descriptor_blob(manifest.config())?;
        for layer in manifest.layers() {
            self.verify_descriptor_blob(layer)?;
        }
        if let Some(subject) = manifest.subject() {
            self.verify_subject_manifest_closure(subject, visited)?;
        }
        Ok(())
    }

    fn verify_descriptor_blob(&self, descriptor: &Descriptor) -> Result<()> {
        let digest = descriptor.digest().clone();
        let descriptor = self
            .stored_descriptor(descriptor.clone())
            .with_context(|| format!("Manifest closure blob is missing or invalid: {digest}"))?;
        self.get_blob(&descriptor)
            .with_context(|| format!("Manifest closure blob is missing or invalid: {digest}"))?;
        Ok(())
    }

    fn verify_subject_manifest_closure(
        &self,
        descriptor: &Descriptor,
        visited: &mut BTreeSet<String>,
    ) -> Result<()> {
        if !visited.insert(descriptor.digest().as_ref().to_string()) {
            return Ok(());
        }
        let digest = descriptor.digest().clone();
        let descriptor = self
            .stored_descriptor(descriptor.clone())
            .with_context(|| format!("Subject manifest is missing or invalid: {digest}"))?;
        let bytes = self
            .get_blob(&descriptor)
            .with_context(|| format!("Subject manifest is missing or invalid: {digest}"))?;
        let manifest: ImageManifest = serde_json::from_slice(&bytes).with_context(|| {
            format!(
                "Failed to parse subject OCI image manifest {}",
                descriptor.digest()
            )
        })?;
        self.verify_manifest_dependencies(&manifest, visited)
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
        let artifact = self.artifact_manifest_record(sealed_artifact.digest())?;
        self.index
            .publish_artifact_ref(image_name, &sealed_artifact.0, &artifact)
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

    /// Publish an existing, complete manifest closure under an image ref.
    ///
    /// Existing closures can contain old blobs that are immediately eligible
    /// for GC. Validation and publication therefore share the same
    /// cross-process exclusion boundary as the deleting GC pass. Crate-visible
    /// for `LocalArtifact::tag_as`; the verified-closure capability remains
    /// private to the Local Registry owner.
    pub(crate) fn publish_existing_manifest_ref(
        &self,
        image_name: &ImageRef,
        manifest_digest: &Digest,
    ) -> Result<RefUpdate> {
        let _gc_exclusion = self.lock_gc_exclusion()?;
        let verified = self.verify_manifest_closure(image_name, manifest_digest)?;
        match verified.projection {
            VerifiedManifestProjection::Artifact(artifact) => {
                self.index
                    .publish_artifact_ref(image_name, &verified.manifest, &artifact)
            }
            VerifiedManifestProjection::Experiment(experiment) => self
                .index
                .publish_experiment_ref(image_name, &verified.manifest, &experiment),
        }
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
        let artifact = self.artifact_manifest_record(sealed_artifact.digest())?;
        self.index
            .replace_artifact_ref(image_name, &sealed_artifact.0, &artifact)
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

    /// Remove a local image ref without deleting content-addressed blobs.
    ///
    /// The mutable `(name, reference) -> manifest digest` row is removed from
    /// this Local Registry. The immutable manifest, config, and layer blobs are
    /// reclaimed only when a later [`Self::gc`] pass finds them unreachable.
    /// Returns the removed ref record, including the manifest digest needed by
    /// [`Self::restore_image_ref`], or `None` when the ref did not exist.
    pub fn remove_image_ref(
        &self,
        image_name: &ImageRef,
    ) -> Result<Option<crate::artifact::local_registry::RefRecord>> {
        self.index
            .delete_ref(&image_name.repository_key(), image_name.reference())
    }

    /// Restore an image ref to a manifest that remains in this Local Registry.
    ///
    /// The manifest and its complete config/layer/subject closure are verified
    /// from the CAS before the ref is published. Verification and publication
    /// are serialized against deleting GC passes across processes. An existing
    /// ref at the same name is never moved: the returned
    /// [`RefUpdate::Conflicted`] identifies a different current target, while
    /// an identical target returns [`RefUpdate::Unchanged`].
    pub fn restore_image_ref(
        &self,
        image_name: &ImageRef,
        manifest_digest: &Digest,
    ) -> Result<RefUpdate> {
        self.publish_existing_manifest_ref(image_name, manifest_digest)
    }

    /// Exclude deletion GC while an existing CAS closure is validated and
    /// published. The file lock coordinates independent registry instances and
    /// processes; SQLite and blob-store locks remain responsible for their
    /// respective local mutations inside this owner-level boundary.
    fn lock_gc_exclusion(&self) -> Result<GcExclusionGuard> {
        let path = self.root.join(GC_EXCLUSION_LOCK_FILE_NAME);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .with_context(|| format!("Failed to open GC exclusion lock {}", path.display()))?;
        file.lock()
            .with_context(|| format!("Failed to lock GC exclusion at {}", path.display()))?;
        Ok(GcExclusionGuard { file })
    }

    fn store_blob_bytes(&self, bytes: &[u8]) -> Result<Digest> {
        self.blobs.put_bytes(bytes)
    }

    fn store_blob_reader(&self, reader: impl Read) -> Result<(Digest, u64)> {
        self.blobs.put_reader(reader)
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

    /// Validate that the manifest carries the OMMX `artifactType`.
    fn validate_manifest(manifest: &ImageManifest) -> Result<()> {
        let artifact_type = manifest
            .artifact_type()
            .as_ref()
            .context("Manifest does not carry the OMMX `artifactType` field")?;
        ensure!(
            media_types::is_ommx_artifact_type(artifact_type),
            "Manifest `artifactType` must be one of `{}` or `{}`, got `{}`",
            media_types::V1_ARTIFACT_MEDIA_TYPE,
            media_types::V1_EXPERIMENT_MEDIA_TYPE,
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
        attachment_storage::store_layer_bytes(self, media_type, bytes, annotations)
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
