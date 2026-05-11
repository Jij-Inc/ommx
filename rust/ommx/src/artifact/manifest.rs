use super::{
    digest::sha256_digest,
    ghcr,
    local_registry::{LocalRegistry, RefConflictPolicy, RefUpdate},
    media_types::{self, OCI_EMPTY_CONFIG_BYTES},
    InstanceAnnotations, ParametricInstanceAnnotations, SampleSetAnnotations, SolutionAnnotations,
};
use crate::v1;
use anyhow::{bail, Context, Result};
use oci_spec::image::{
    Descriptor, DescriptorBuilder, Digest, ImageManifest,
    ImageManifestBuilder as OciImageManifestBuilder, MediaType,
};
use prost::Message;
use serde::Serialize;
use std::{
    collections::HashMap,
    str::FromStr,
    sync::{Arc, OnceLock},
};
use url::Url;

/// A blob whose `Descriptor` has already been computed and that is
/// staged in memory for the next `publish_artifact_manifest` call.
///
/// Bridges the in-memory `LocalArtifactBuilder` (Build phase) and the
/// I/O-side registry publish (Seal phase) — the analogue of a Git
/// blob entry sitting in the index after `git add`, before `git
/// commit` writes it to `.git/objects/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StagedArtifactBlob {
    descriptor: Descriptor,
    bytes: Vec<u8>,
}

impl StagedArtifactBlob {
    pub(crate) fn new(
        media_type: MediaType,
        bytes: Vec<u8>,
        annotations: HashMap<String, String>,
    ) -> Result<Self> {
        let descriptor = descriptor_from_bytes(media_type, &bytes, annotations)?;
        Ok(Self { descriptor, bytes })
    }

    pub(crate) fn descriptor(&self) -> &Descriptor {
        &self.descriptor
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// OMMX Artifact stored in the SQLite-backed Local Registry.
///
/// Holds an [`Arc`]ed [`LocalRegistry`] so that several artifacts opened
/// from the same registry share a single SQLite connection and blob
/// store handle. Combined with the `Mutex<Connection>` inside
/// [`super::local_registry::SqliteIndexStore`], this makes
/// `LocalArtifact` `Sync` and `Clone`-friendly without any per-artifact
/// connection duplication.
///
/// The parsed manifest is memoised in an `Arc<OnceLock<LocalManifest>>`
/// so repeated calls to [`Self::layers`] / [`Self::annotations`] /
/// [`Self::subject`] do not re-read the manifest blob from the
/// `BlobStore`, requery the `IndexStore`, and re-parse the JSON each
/// time. Clones of the artifact share the same cell, so any clone that
/// reads the manifest first warms it for the rest.
#[derive(Debug, Clone)]
pub struct LocalArtifact {
    registry: Arc<LocalRegistry>,
    image_name: ocipkg::ImageName,
    manifest_digest: String,
    manifest_cache: Arc<OnceLock<LocalManifest>>,
}

impl LocalArtifact {
    pub(crate) fn from_parts(
        registry: Arc<LocalRegistry>,
        image_name: ocipkg::ImageName,
        manifest_digest: String,
    ) -> Self {
        Self {
            registry,
            image_name,
            manifest_digest,
            manifest_cache: Arc::new(OnceLock::new()),
        }
    }

    pub fn open(image_name: ocipkg::ImageName) -> Result<Self> {
        let registry = Arc::new(LocalRegistry::open_default()?);
        Self::open_in_registry(registry, image_name)
    }

    pub fn open_in_registry(
        registry: Arc<LocalRegistry>,
        image_name: ocipkg::ImageName,
    ) -> Result<Self> {
        Self::try_open_in_registry(registry, image_name.clone())?.with_context(|| {
            format!(
                "Artifact not found in the SQLite-backed local registry: {image_name}. \
                 If this artifact exists in the legacy OCI directory local registry, \
                 run `ommx artifact import` once, then retry."
            )
        })
    }

    pub fn try_open(image_name: ocipkg::ImageName) -> Result<Option<Self>> {
        let registry = Arc::new(LocalRegistry::open_default()?);
        Self::try_open_in_registry(registry, image_name)
    }

    pub fn try_open_in_registry(
        registry: Arc<LocalRegistry>,
        image_name: ocipkg::ImageName,
    ) -> Result<Option<Self>> {
        let Some(manifest_digest) = registry.resolve_image_name(&image_name)? else {
            return Ok(None);
        };
        Ok(Some(Self::from_parts(
            registry,
            image_name,
            manifest_digest,
        )))
    }

    pub fn image_name(&self) -> &ocipkg::ImageName {
        &self.image_name
    }

    pub fn manifest_digest(&self) -> &str {
        &self.manifest_digest
    }

    /// Root path of the SQLite Local Registry this artifact lives in.
    ///
    /// Public so that the Python binding can derive the matching legacy
    /// OCI dir path (`registry_root.join(image_name.as_path())`) for
    /// the transitional `push()` path that round-trips through ocipkg.
    /// Goes away once native v3 push lands.
    pub fn registry_root(&self) -> &std::path::Path {
        self.registry.root()
    }

    /// Read and cache the manifest associated with this artifact.
    ///
    /// The first successful call populates a shared `OnceLock`; later
    /// calls (and clones of `self`) reuse the cached value without
    /// touching the `BlobStore` / `IndexStore`. Failed reads are not
    /// cached, so transient errors retry on the next call.
    pub fn get_manifest(&self) -> Result<&LocalManifest> {
        if let Some(cached) = self.manifest_cache.get() {
            return Ok(cached);
        }
        let manifest = self.read_manifest_uncached()?;
        Ok(self.manifest_cache.get_or_init(|| manifest))
    }

    fn read_manifest_uncached(&self) -> Result<LocalManifest> {
        // Verify the IndexStore has a manifest record for this digest so a
        // missing index entry surfaces as a clear "manifest not found"
        // error instead of bubbling up as a parse failure or stale-cache
        // hit. The record's `media_type` column is informational — the
        // SQLite Local Registry only stores OCI Image Manifest, so no
        // per-call dispatch is needed.
        self.registry
            .index()
            .get_manifest(&self.manifest_digest)?
            .with_context(|| {
                format!(
                    "Manifest record {} not found in IndexStore",
                    self.manifest_digest
                )
            })?;
        let bytes = self.registry.blobs().read_bytes(&self.manifest_digest)?;
        LocalManifest::parse(&bytes)
    }

    pub fn annotations(&self) -> Result<HashMap<String, String>> {
        Ok(self.get_manifest()?.annotations())
    }

    pub fn layers(&self) -> Result<Vec<Descriptor>> {
        Ok(self.get_manifest()?.layers())
    }

    pub fn subject(&self) -> Result<Option<Descriptor>> {
        Ok(self.get_manifest()?.subject())
    }

    pub fn get_blob(&self, digest: &str) -> Result<Vec<u8>> {
        self.registry.blobs().read_bytes(digest)
    }
}

/// A manifest read from the SQLite Local Registry. v3 stores OCI Image
/// Manifest as the only native format; OMMX artifacts are identified
/// by the `artifactType` field plus the `application/vnd.oci.empty.v1+json`
/// empty config descriptor. The deprecated OCI Artifact Manifest media
/// type (`application/vnd.oci.artifact.manifest.v1+json`) is rejected
/// at parse time rather than supported via a second enum variant.
///
/// Boxed because `oci_spec`'s `ImageManifest` is large (~800 bytes) and
/// callers move `LocalManifest` around inside `Arc<OnceLock<...>>`.
#[derive(Debug, Clone)]
pub struct LocalManifest(Box<ImageManifest>);

impl LocalManifest {
    fn parse(bytes: &[u8]) -> Result<Self> {
        let manifest: ImageManifest =
            serde_json::from_slice(bytes).context("Failed to parse OCI image manifest")?;
        ensure_ommx_image_manifest(&manifest)?;
        Ok(Self(Box::new(manifest)))
    }

    pub fn media_type(&self) -> &'static str {
        media_types::OCI_IMAGE_MANIFEST_MEDIA_TYPE
    }

    /// Always returns the OMMX `artifactType` discriminator. `parse`
    /// rejects manifests without one (see `ensure_ommx_image_manifest`),
    /// so this method does not surface an `Option`.
    pub fn artifact_type(&self) -> &MediaType {
        self.0
            .artifact_type()
            .as_ref()
            .expect("ensure_ommx_image_manifest guarantees Image Manifest carries an artifactType")
    }

    pub fn layers(&self) -> Vec<Descriptor> {
        self.0.layers().to_vec()
    }

    pub fn annotations(&self) -> HashMap<String, String> {
        self.0.annotations().clone().unwrap_or_default()
    }

    pub fn subject(&self) -> Option<Descriptor> {
        self.0.subject().clone()
    }
}

/// Builder for OMMX Artifacts stored in the SQLite-backed Local Registry.
///
/// Produces an OCI Image Manifest with `artifactType` set to the OMMX
/// artifact media type and the OCI 1.1 empty config descriptor as
/// `config`. The layer blobs land in the Image Manifest's `layers[]`
/// field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalArtifactBuilder {
    image_name: ocipkg::ImageName,
    artifact_type: MediaType,
    layers: Vec<StagedArtifactBlob>,
    subject: Option<Descriptor>,
    annotations: HashMap<String, String>,
}

impl LocalArtifactBuilder {
    pub fn new(image_name: ocipkg::ImageName) -> Self {
        Self {
            image_name,
            artifact_type: MediaType::Other(media_types::V1_ARTIFACT_MEDIA_TYPE.to_string()),
            layers: Vec::new(),
            subject: None,
            annotations: HashMap::new(),
        }
    }

    /// Create a new artifact builder for a GitHub container registry image name.
    pub fn for_github(org: &str, repo: &str, name: &str, tag: &str) -> Result<Self> {
        let image_name = ghcr(org, repo, name, tag)?;
        let source = Url::parse(&format!("https://github.com/{org}/{repo}"))?;

        let mut builder = Self::new(image_name);
        builder.add_source(&source);
        Ok(builder)
    }

    pub fn add_layer_bytes(
        &mut self,
        media_type: MediaType,
        bytes: Vec<u8>,
        annotations: HashMap<String, String>,
    ) -> Result<Descriptor> {
        let blob = StagedArtifactBlob::new(media_type, bytes, annotations)?;
        let descriptor = blob.descriptor.clone();
        self.layers.push(blob);
        Ok(descriptor)
    }

    pub fn add_instance(
        &mut self,
        instance: v1::Instance,
        annotations: InstanceAnnotations,
    ) -> Result<Descriptor> {
        self.add_layer_bytes(
            media_types::v1_instance(),
            instance.encode_to_vec(),
            annotations.into(),
        )
    }

    pub fn add_solution(
        &mut self,
        solution: v1::State,
        annotations: SolutionAnnotations,
    ) -> Result<Descriptor> {
        self.add_layer_bytes(
            media_types::v1_solution(),
            solution.encode_to_vec(),
            annotations.into(),
        )
    }

    pub fn add_parametric_instance(
        &mut self,
        instance: v1::ParametricInstance,
        annotations: ParametricInstanceAnnotations,
    ) -> Result<Descriptor> {
        self.add_layer_bytes(
            media_types::v1_parametric_instance(),
            instance.encode_to_vec(),
            annotations.into(),
        )
    }

    pub fn add_sample_set(
        &mut self,
        sample_set: v1::SampleSet,
        annotations: SampleSetAnnotations,
    ) -> Result<Descriptor> {
        self.add_layer_bytes(
            media_types::v1_sample_set(),
            sample_set.encode_to_vec(),
            annotations.into(),
        )
    }

    pub fn set_subject(&mut self, subject: Descriptor) -> &mut Self {
        self.subject = Some(subject);
        self
    }

    pub fn add_annotation(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.annotations.insert(key.into(), value.into());
    }

    pub fn add_source(&mut self, url: &Url) {
        self.add_annotation("org.opencontainers.image.source", url.to_string());
    }

    pub fn build(self) -> Result<LocalArtifact> {
        let registry = Arc::new(LocalRegistry::open_default()?);
        self.build_in_registry(registry, RefConflictPolicy::KeepExisting)
    }

    pub fn build_in_registry(
        self,
        registry: Arc<LocalRegistry>,
        policy: RefConflictPolicy,
    ) -> Result<LocalArtifact> {
        let staged = self.stage()?;
        let ref_update = registry.publish_artifact_manifest(
            &staged.image_name,
            &staged.manifest,
            &staged.manifest_descriptor,
            &staged.manifest_bytes,
            &staged.blobs,
            policy,
        )?;
        reject_conflicting_ref(&staged.image_name, ref_update)?;
        Ok(LocalArtifact::from_parts(
            registry,
            staged.image_name,
            staged.manifest_descriptor.digest().to_string(),
        ))
    }

    /// Compute the manifest, its stable JSON bytes, and the matching
    /// descriptor from the in-memory builder state, returning a
    /// [`StagedArtifactManifest`] that the registry can later publish.
    /// Pure: no I/O, no registry interaction.
    ///
    /// Materialises the OCI 1.1 empty config blob as one of the staged
    /// blobs so the publish path uploads it alongside the layers. The
    /// caller does not need to know about empty-config bookkeeping.
    fn stage(self) -> Result<StagedArtifactManifest> {
        let mut blobs = self.layers;
        let config_blob = StagedArtifactBlob::new(
            MediaType::EmptyJSON,
            OCI_EMPTY_CONFIG_BYTES.to_vec(),
            HashMap::new(),
        )?;
        let config_descriptor = config_blob.descriptor.clone();
        let layer_descriptors: Vec<Descriptor> =
            blobs.iter().map(|blob| blob.descriptor.clone()).collect();
        blobs.push(config_blob);

        let mut builder = OciImageManifestBuilder::default()
            .schema_version(2u32)
            .media_type(MediaType::ImageManifest)
            .artifact_type(self.artifact_type)
            .config(config_descriptor)
            .layers(layer_descriptors);
        if let Some(subject) = self.subject {
            builder = builder.subject(subject);
        }
        if !self.annotations.is_empty() {
            builder = builder.annotations(self.annotations);
        }
        let manifest = builder
            .build()
            .context("Failed to build OCI image manifest")?;
        let manifest_bytes = stable_json_bytes(&manifest)?;
        let manifest_descriptor =
            descriptor_from_bytes(MediaType::ImageManifest, &manifest_bytes, HashMap::new())?;
        Ok(StagedArtifactManifest {
            image_name: self.image_name,
            manifest,
            manifest_bytes,
            manifest_descriptor,
            blobs,
        })
    }
}

/// The whole-artifact analogue of [`StagedArtifactBlob`]: bundles the
/// `OCI ImageManifest` together with its stable JSON bytes, the
/// matching `Descriptor` (digest / size / media type), every blob
/// staged for upload (layers + the OCI 1.1 empty config), and the
/// target `ImageName`.
///
/// Produced purely by in-memory computation (`LocalArtifactBuilder::stage`)
/// and consumed by the registry publish path
/// (`LocalRegistry::publish_artifact_manifest`). Splitting the Build
/// phase ("compute everything we need") from the Seal phase ("write
/// blobs / insert manifest record / update ref atomically") keeps
/// publish a pure I/O step that only validates the staged bundle.
///
/// The Git analogy is the constructed-but-not-yet-written tree +
/// commit object — a `git commit` materialises objects in
/// `.git/objects/` and updates `refs/heads/<branch>`; here, publish
/// materialises CAS bytes and updates the IndexStore ref to the new
/// manifest digest.
#[derive(Debug, Clone, PartialEq, Eq)]
struct StagedArtifactManifest {
    image_name: ocipkg::ImageName,
    manifest: ImageManifest,
    manifest_bytes: Vec<u8>,
    manifest_descriptor: Descriptor,
    blobs: Vec<StagedArtifactBlob>,
}

pub(crate) fn descriptor_from_bytes(
    media_type: MediaType,
    bytes: &[u8],
    annotations: HashMap<String, String>,
) -> Result<Descriptor> {
    let digest = Digest::from_str(&sha256_digest(bytes)).context("Failed to parse blob digest")?;
    let mut builder = DescriptorBuilder::default()
        .media_type(media_type)
        .digest(digest)
        .size(bytes.len() as u64);
    if !annotations.is_empty() {
        builder = builder.annotations(annotations);
    }
    builder.build().context("Failed to build OCI descriptor")
}

pub(crate) fn stable_json_bytes(value: &impl Serialize) -> Result<Vec<u8>> {
    let mut value = serde_json::to_value(value).context("Failed to encode JSON value")?;
    value.sort_all_objects();
    serde_json::to_vec(&value).context("Failed to encode stable JSON bytes")
}

fn reject_conflicting_ref(image_name: &ocipkg::ImageName, ref_update: RefUpdate) -> Result<()> {
    if let RefUpdate::Conflicted {
        existing_manifest_digest,
        incoming_manifest_digest,
    } = ref_update
    {
        bail!(
            "Local registry ref {image_name} already points to {existing_manifest_digest}; \
             incoming manifest {incoming_manifest_digest} was not published"
        );
    }
    Ok(())
}

fn ensure_ommx_image_manifest(manifest: &ImageManifest) -> Result<()> {
    let artifact_type = manifest
        .artifact_type()
        .as_ref()
        .context("OCI image manifest is not an OMMX artifact: artifactType is missing")?;
    anyhow::ensure!(
        artifact_type == &media_types::v1_artifact(),
        "OCI image manifest is not an OMMX artifact: {artifact_type}",
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_image_name(tag: &str) -> Result<ocipkg::ImageName> {
        ocipkg::ImageName::parse(&format!("ghcr.io/jij-inc/ommx/demo:{tag}"))
    }

    #[test]
    fn builds_native_oci_image_manifest_with_artifact_type() -> Result<()> {
        let mut builder = LocalArtifactBuilder::new(test_image_name("v1")?);
        let layer = builder.add_layer_bytes(
            MediaType::Other(media_types::V1_INSTANCE_MEDIA_TYPE.to_string()),
            b"instance".to_vec(),
            HashMap::from([("org.ommx.v1.instance.title".to_string(), "demo".to_string())]),
        )?;
        builder.add_annotation("org.opencontainers.image.ref.name", "example.com/demo:v1");

        let staged = builder.stage()?;
        assert_eq!(
            staged.manifest.media_type().as_ref(),
            Some(&MediaType::ImageManifest)
        );
        assert_eq!(
            staged.manifest.artifact_type().as_ref(),
            Some(&MediaType::Other(
                media_types::V1_ARTIFACT_MEDIA_TYPE.to_string()
            ))
        );
        assert_eq!(staged.manifest.layers(), &[layer]);

        // OCI 1.1 empty config descriptor is set as the manifest's config and
        // staged for upload alongside the layers.
        let config = staged.manifest.config();
        assert_eq!(config.media_type(), &MediaType::EmptyJSON);
        assert_eq!(
            config.size(),
            media_types::OCI_EMPTY_CONFIG_BYTES.len() as u64
        );
        assert_eq!(
            config.digest().to_string(),
            media_types::OCI_EMPTY_CONFIG_DIGEST
        );
        assert!(staged
            .blobs
            .iter()
            .any(|blob| blob.descriptor.digest() == config.digest()));

        assert_eq!(
            staged.manifest_descriptor.media_type(),
            &MediaType::ImageManifest
        );
        assert_eq!(
            staged.manifest_descriptor.digest().to_string(),
            sha256_digest(&staged.manifest_bytes)
        );

        let parsed: ImageManifest = serde_json::from_slice(&staged.manifest_bytes)?;
        assert_eq!(parsed, staged.manifest);
        Ok(())
    }

    #[test]
    fn stable_manifest_json_is_independent_of_annotation_insertion_order() -> Result<()> {
        let first = staged_with_annotations("order-a", [("b", "2"), ("a", "1")])?;
        let second = staged_with_annotations("order-b", [("a", "1"), ("b", "2")])?;

        assert_eq!(first.manifest_bytes, second.manifest_bytes);
        assert_eq!(
            first.manifest_descriptor.digest(),
            second.manifest_descriptor.digest()
        );
        Ok(())
    }

    #[test]
    fn builds_manifest_with_subject() -> Result<()> {
        let subject =
            descriptor_from_bytes(MediaType::ImageManifest, b"parent manifest", HashMap::new())?;
        let mut builder = LocalArtifactBuilder::new(test_image_name("subject")?);
        builder.add_layer_bytes(
            MediaType::Other(media_types::V1_INSTANCE_MEDIA_TYPE.to_string()),
            b"instance".to_vec(),
            HashMap::new(),
        )?;
        builder.set_subject(subject.clone());

        let staged = builder.stage()?;
        assert_eq!(staged.manifest.subject(), &Some(subject));
        Ok(())
    }

    #[test]
    fn rejects_invalid_descriptor_digest_through_oci_spec() {
        assert!(Digest::from_str("sha256:../bad").is_err());
    }

    fn staged_with_annotations(
        tag: &str,
        annotations: impl IntoIterator<Item = (&'static str, &'static str)>,
    ) -> Result<StagedArtifactManifest> {
        let mut builder = LocalArtifactBuilder::new(test_image_name(tag)?);
        builder.add_layer_bytes(
            MediaType::Other(media_types::V1_INSTANCE_MEDIA_TYPE.to_string()),
            b"instance".to_vec(),
            HashMap::new(),
        )?;
        for (key, value) in annotations {
            builder.add_annotation(key, value);
        }
        builder.stage()
    }
}
