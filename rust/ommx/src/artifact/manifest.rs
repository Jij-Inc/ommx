use super::{
    digest::sha256_digest,
    ghcr,
    local_registry::{LocalRegistry, RefConflictPolicy, RefUpdate},
    media_types::{self, OCI_ARTIFACT_MANIFEST_MEDIA_TYPE, OCI_IMAGE_MANIFEST_MEDIA_TYPE},
    InstanceAnnotations, ParametricInstanceAnnotations, SampleSetAnnotations, SolutionAnnotations,
};
use crate::v1;
use anyhow::{bail, Context, Result};
use oci_spec::image::{
    ArtifactManifest, ArtifactManifestBuilder as OciArtifactManifestBuilder, Descriptor,
    DescriptorBuilder, Digest, ImageManifest, MediaType,
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

    /// Borrow the underlying registry, which may be shared with other
    /// `LocalArtifact` instances.
    pub fn registry(&self) -> &Arc<LocalRegistry> {
        &self.registry
    }

    pub fn image_name(&self) -> &ocipkg::ImageName {
        &self.image_name
    }

    pub fn manifest_digest(&self) -> &str {
        &self.manifest_digest
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
        let bytes = self.registry.blobs().read_bytes(&self.manifest_digest)?;
        let record = self
            .registry
            .index()
            .get_manifest(&self.manifest_digest)?
            .with_context(|| {
                format!(
                    "Manifest record {} not found in IndexStore",
                    self.manifest_digest
                )
            })?;
        LocalManifest::parse(&record.media_type, &bytes)
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

/// A manifest read from the SQLite Local Registry, dispatched on its OCI media
/// type. v3 stores both Image Manifest (legacy import path) and Artifact
/// Manifest (native build path) and identifies each by its bytes digest.
///
/// Both variants are boxed because `oci_spec`'s `ImageManifest` (~800 bytes)
/// and `ArtifactManifest` (~460 bytes) are large structs — keeping them
/// inline would either trip `clippy::large_enum_variant` or pad the smaller
/// variant up to the larger one. The boxed indirection costs one allocation
/// per `get_manifest()` call but keeps both the enum and the surrounding
/// `Result<LocalManifest>` cheap to move around.
#[derive(Debug, Clone)]
pub enum LocalManifest {
    Image(Box<ImageManifest>),
    Artifact(Box<ArtifactManifest>),
}

impl LocalManifest {
    fn parse(media_type: &str, bytes: &[u8]) -> Result<Self> {
        match media_type {
            OCI_ARTIFACT_MANIFEST_MEDIA_TYPE => {
                let manifest: ArtifactManifest = serde_json::from_slice(bytes)
                    .context("Failed to parse OCI artifact manifest")?;
                ensure_ommx_artifact_manifest(&manifest)?;
                Ok(LocalManifest::Artifact(Box::new(manifest)))
            }
            OCI_IMAGE_MANIFEST_MEDIA_TYPE => {
                let manifest: ImageManifest =
                    serde_json::from_slice(bytes).context("Failed to parse OCI image manifest")?;
                ensure_ommx_image_manifest(&manifest)?;
                Ok(LocalManifest::Image(Box::new(manifest)))
            }
            other => bail!("Unsupported manifest media type for OMMX artifact: {other}"),
        }
    }

    pub fn media_type(&self) -> &'static str {
        match self {
            LocalManifest::Image(_) => OCI_IMAGE_MANIFEST_MEDIA_TYPE,
            LocalManifest::Artifact(_) => OCI_ARTIFACT_MANIFEST_MEDIA_TYPE,
        }
    }

    /// Always returns the OMMX `artifactType` discriminator. `parse`
    /// rejects manifests without one (see `ensure_ommx_image_manifest`
    /// / `ensure_ommx_artifact_manifest`), so this method does not
    /// surface an `Option`.
    pub fn artifact_type(&self) -> &MediaType {
        match self {
            LocalManifest::Image(m) => m.artifact_type().as_ref().expect(
                "ensure_ommx_image_manifest guarantees Image Manifest carries an artifactType",
            ),
            LocalManifest::Artifact(m) => m.artifact_type(),
        }
    }

    pub fn layers(&self) -> Vec<Descriptor> {
        match self {
            LocalManifest::Image(m) => m.layers().to_vec(),
            LocalManifest::Artifact(m) => m.blobs().to_vec(),
        }
    }

    pub fn annotations(&self) -> HashMap<String, String> {
        let raw = match self {
            LocalManifest::Image(m) => m.annotations().clone(),
            LocalManifest::Artifact(m) => m.annotations().clone(),
        };
        raw.unwrap_or_default()
    }

    pub fn subject(&self) -> Option<Descriptor> {
        match self {
            LocalManifest::Image(m) => m.subject().clone(),
            LocalManifest::Artifact(m) => m.subject().clone(),
        }
    }
}

/// Builder for OMMX Artifacts stored in the SQLite-backed Local Registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalArtifactBuilder {
    image_name: ocipkg::ImageName,
    artifact_type: MediaType,
    blobs: Vec<StagedArtifactBlob>,
    subject: Option<Descriptor>,
    annotations: HashMap<String, String>,
}

impl LocalArtifactBuilder {
    pub fn new(image_name: ocipkg::ImageName, artifact_type: MediaType) -> Self {
        Self {
            image_name,
            artifact_type,
            blobs: Vec::new(),
            subject: None,
            annotations: HashMap::new(),
        }
    }

    pub fn new_ommx(image_name: ocipkg::ImageName) -> Self {
        Self::new(
            image_name,
            MediaType::Other(media_types::V1_ARTIFACT_MEDIA_TYPE.to_string()),
        )
    }

    /// Create a new artifact builder for a GitHub container registry image name.
    pub fn for_github(org: &str, repo: &str, name: &str, tag: &str) -> Result<Self> {
        let image_name = ghcr(org, repo, name, tag)?;
        let source = Url::parse(&format!("https://github.com/{org}/{repo}"))?;

        let mut builder = Self::new_ommx(image_name);
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
        self.blobs.push(blob);
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
    fn stage(self) -> Result<StagedArtifactManifest> {
        let mut builder = OciArtifactManifestBuilder::default()
            .artifact_type(self.artifact_type)
            .blobs(
                self.blobs
                    .iter()
                    .map(|blob| blob.descriptor.clone())
                    .collect::<Vec<_>>(),
            );
        if let Some(subject) = self.subject {
            builder = builder.subject(subject);
        }
        if !self.annotations.is_empty() {
            builder = builder.annotations(self.annotations);
        }
        let manifest = builder
            .build()
            .context("Failed to build OCI artifact manifest")?;
        let manifest_bytes = stable_json_bytes(&manifest)?;
        let manifest_descriptor =
            descriptor_from_bytes(MediaType::ArtifactManifest, &manifest_bytes, HashMap::new())?;
        Ok(StagedArtifactManifest {
            image_name: self.image_name,
            manifest,
            manifest_bytes,
            manifest_descriptor,
            blobs: self.blobs,
        })
    }
}

/// The whole-artifact analogue of [`StagedArtifactBlob`]: bundles the
/// `OCI ArtifactManifest` together with its stable JSON bytes, the
/// matching `Descriptor` (digest / size / media type), every layer
/// blob staged for upload, and the target `ImageName`.
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
    manifest: ArtifactManifest,
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

fn ensure_ommx_artifact_manifest(manifest: &ArtifactManifest) -> Result<()> {
    anyhow::ensure!(
        manifest.media_type().as_ref() == OCI_ARTIFACT_MANIFEST_MEDIA_TYPE,
        "Manifest is not an OCI artifact manifest: {}",
        manifest.media_type()
    );
    anyhow::ensure!(
        manifest.artifact_type() == &media_types::v1_artifact(),
        "Not an OMMX Artifact: {}",
        manifest.artifact_type()
    );
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
    fn builds_native_oci_artifact_manifest() -> Result<()> {
        let mut builder = LocalArtifactBuilder::new_ommx(test_image_name("v1")?);
        let blob = builder.add_layer_bytes(
            MediaType::Other(media_types::V1_INSTANCE_MEDIA_TYPE.to_string()),
            b"instance".to_vec(),
            HashMap::from([("org.ommx.v1.instance.title".to_string(), "demo".to_string())]),
        )?;
        builder.add_annotation("org.opencontainers.image.ref.name", "example.com/demo:v1");

        let staged = builder.stage()?;
        assert_eq!(staged.manifest.media_type(), &MediaType::ArtifactManifest);
        assert_eq!(
            staged.manifest.artifact_type(),
            &MediaType::Other(media_types::V1_ARTIFACT_MEDIA_TYPE.to_string())
        );
        assert_eq!(staged.manifest.blobs(), &[blob]);
        assert_eq!(
            staged.manifest_descriptor.media_type(),
            &MediaType::ArtifactManifest
        );
        assert_eq!(
            staged.manifest_descriptor.digest().to_string(),
            sha256_digest(&staged.manifest_bytes)
        );

        let parsed: ArtifactManifest = serde_json::from_slice(&staged.manifest_bytes)?;
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
        let subject = descriptor_from_bytes(
            MediaType::ArtifactManifest,
            b"parent manifest",
            HashMap::new(),
        )?;
        let mut builder = LocalArtifactBuilder::new_ommx(test_image_name("subject")?);
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
        let mut builder = LocalArtifactBuilder::new_ommx(test_image_name(tag)?);
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
