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
        // hit. The record's `media_type` column is informational for the
        // common Image Manifest case, but is also used here to detect
        // entries written by earlier v3-alpha builds (`#864` / `#866`)
        // as OCI Artifact Manifest and surface a targeted error instead
        // of an opaque image-manifest parse failure.
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
        if record.media_type != media_types::OCI_IMAGE_MANIFEST_MEDIA_TYPE {
            bail!(
                "Manifest {} was persisted as `{}`, which is not supported in this build. \
                 Only OCI Image Manifest (`{}`) is read from the SQLite Local Registry.",
                self.manifest_digest,
                record.media_type,
                media_types::OCI_IMAGE_MANIFEST_MEDIA_TYPE,
            );
        }
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
/// at parse time by the `artifactType` field (validated against
/// `application/org.ommx.v1.artifact`). The native build path also
/// writes an `application/vnd.oci.empty.v1+json` empty config descriptor
/// — matching the SDK v2 archive build — but `parse` does not assert
/// on the config blob, so legacy v2 imports that carry an OMMX-specific
/// config remain readable. The deprecated OCI Artifact Manifest media
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

    pub fn config(&self) -> Descriptor {
        self.0.config().clone()
    }

    pub fn layers(&self) -> Vec<Descriptor> {
        self.0.layers().to_vec()
    }

    /// Consume this wrapper and return the inner OCI Image Manifest.
    /// Used by callers that need to round-trip the manifest as JSON
    /// (e.g. the CLI's `ommx inspect`), where the standard OCI
    /// `serde` form is what's expected. The accessors above cover
    /// programmatic use; `into_inner` is the escape hatch when the
    /// whole thing needs to leave the wrapper.
    pub fn into_inner(self) -> ImageManifest {
        *self.0
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
/// How the builder picks its image name at build time. Explicit
/// values get used verbatim; the `Anonymous` variant lets
/// [`LocalArtifactBuilder::build_in_registry`] synthesize a name
/// against the actual target registry's `registry_id` instead of
/// committing to one at construction time. Building an anonymous
/// builder into a fresh `LocalRegistry` therefore stamps the
/// destination-registry's id into the synthesized hostname, not the
/// default registry's id.
#[derive(Debug, Clone, PartialEq, Eq)]
enum BuilderImageName {
    Explicit(ocipkg::ImageName),
    Anonymous,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalArtifactBuilder {
    image_name: BuilderImageName,
    artifact_type: MediaType,
    layers: Vec<StagedArtifactBlob>,
    subject: Option<Descriptor>,
    annotations: HashMap<String, String>,
}

impl LocalArtifactBuilder {
    pub fn new(image_name: ocipkg::ImageName) -> Self {
        Self {
            image_name: BuilderImageName::Explicit(image_name),
            artifact_type: MediaType::Other(media_types::V1_ARTIFACT_MEDIA_TYPE.to_string()),
            layers: Vec::new(),
            subject: None,
            annotations: HashMap::new(),
        }
    }

    /// Builder for an artifact whose name is auto-generated. UX
    /// shortcut for "I just want to share this artifact, I don't want
    /// to invent a real name". The synthesized image name has the form
    /// `<registry-id8>.ommx.local/anonymous:<local-timestamp>`; the
    /// registry-id prefix is generated once when each
    /// [`LocalRegistry`] is first created and persisted in its SQLite
    /// metadata, so anonymous artifacts from the same registry share
    /// a prefix and can be told apart from artifacts imported from
    /// another registry. Name synthesis is deferred to
    /// [`Self::build_in_registry`] so the prefix reflects the actual
    /// target registry (not the default registry). Use
    /// `ommx artifact prune-anonymous` to clean accumulated entries.
    pub fn new_anonymous() -> Self {
        Self {
            image_name: BuilderImageName::Anonymous,
            artifact_type: MediaType::Other(media_types::V1_ARTIFACT_MEDIA_TYPE.to_string()),
            layers: Vec::new(),
            subject: None,
            annotations: HashMap::new(),
        }
    }

    /// Builder under a random `ttl.sh/<uuid>:1h` image name. Insecure;
    /// for tests only.
    pub fn temp() -> Result<Self> {
        let id = uuid::Uuid::new_v4();
        let image_name = ocipkg::ImageName::parse(&format!("ttl.sh/{id}:1h"))?;
        Ok(Self::new(image_name))
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
        mut self,
        registry: Arc<LocalRegistry>,
        mut policy: RefConflictPolicy,
    ) -> Result<LocalArtifact> {
        // Resolve a deferred anonymous name against the *actual*
        // target registry's id, so the synthesized hostname prefix
        // matches the destination registry (not the default
        // registry, which `LocalArtifactBuilder::new_anonymous`
        // could not have known at construction time).
        //
        // Anonymous builds are also transparently switched to
        // `RefConflictPolicy::Replace`: two anonymous builds in the
        // same second produce the same `YYYYMMDDTHHMMSS` tag, and the
        // user's intent is "publish under an auto-generated name", so
        // silently overwriting the older ref is more useful than
        // failing with a `KeepExisting` conflict. Named builds keep
        // the caller-supplied policy intact.
        if let BuilderImageName::Anonymous = self.image_name {
            let registry_id = registry.index().registry_id()?;
            self.image_name =
                BuilderImageName::Explicit(anonymous_artifact_image_name(&registry_id)?);
            policy = RefConflictPolicy::Replace;
        }
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
    /// Materialises the empty config blob as one of the staged blobs so
    /// the publish path uploads it alongside the layers. Matches the
    /// SDK v2 / `ArchiveArtifactBuilder` manifest shape (see
    /// `ocipkg::image::OciArtifactBuilder::new`): `schemaVersion: 2` +
    /// `artifactType` + empty config + layers, with the manifest's
    /// own `mediaType` field intentionally absent so `LocalArtifactBuilder`
    /// and the archive build path produce structurally identical
    /// manifests.
    fn stage(self) -> Result<StagedArtifactManifest> {
        let mut blobs = self.layers;
        // V2 SDK's `ocipkg::OciArtifactBuilder::add_empty_json` emits the
        // empty config descriptor without an `annotations` field; build
        // it directly here (bypassing `descriptor_from_bytes`, which
        // always renders `annotations` even when empty for layer
        // descriptors) so the resulting manifest matches v2 byte-for-byte.
        let empty_config_bytes = OCI_EMPTY_CONFIG_BYTES.to_vec();
        let config_descriptor = DescriptorBuilder::default()
            .media_type(MediaType::EmptyJSON)
            .digest(
                Digest::from_str(&sha256_digest(&empty_config_bytes))
                    .context("Failed to parse empty config digest")?,
            )
            .size(empty_config_bytes.len() as u64)
            .build()
            .context("Failed to build empty config descriptor")?;
        let layer_descriptors: Vec<Descriptor> =
            blobs.iter().map(|blob| blob.descriptor.clone()).collect();
        blobs.push(StagedArtifactBlob {
            descriptor: config_descriptor.clone(),
            bytes: empty_config_bytes,
        });

        let mut builder = OciImageManifestBuilder::default()
            .schema_version(2u32)
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
        // `build_in_registry` resolves the `Anonymous` variant before
        // calling `stage()`, so reaching `stage()` with `Anonymous`
        // here is a bug (someone bypassed the resolve step). Surface
        // it as a clear internal error rather than letting it slip
        // through as a mysterious empty-name.
        let image_name = match self.image_name {
            BuilderImageName::Explicit(name) => name,
            BuilderImageName::Anonymous => {
                crate::bail!(
                    "LocalArtifactBuilder::stage called with an unresolved anonymous image \
                     name. Use `build_in_registry` (which resolves the name against the target \
                     registry's id) instead of calling `stage` directly."
                );
            }
        };
        Ok(StagedArtifactManifest {
            image_name,
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
    // `annotations` is always set, even when empty, matching SDK v2 /
    // `ocipkg::OciArtifactBuilder::add_layer` which renders the field as
    // `"annotations": {}` in the manifest JSON. Preserving this shape
    // keeps layer descriptor bytes (and therefore the manifest digest)
    // byte-for-byte compatible with v2 OMMX artifacts.
    let descriptor = DescriptorBuilder::default()
        .media_type(media_type)
        .digest(digest)
        .size(bytes.len() as u64)
        .annotations(annotations)
        .build()
        .context("Failed to build OCI descriptor")?;
    Ok(descriptor)
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

/// Suffix shared by every anonymous artifact repository name. The
/// full SQLite ref name is `<registry-id8>.ommx.local/anonymous`; the
/// registry-id prefix is randomised per registry, so filtering
/// anonymous artifacts uses
/// `name.ends_with(ANONYMOUS_ARTIFACT_REF_NAME_SUFFIX)`.
/// `ommx artifact prune-anonymous` cleans entries from every prefix
/// in the registry, not just the host's own.
///
/// The hostname segment `.ommx.local` deliberately uses the `.local`
/// link-local TLD (RFC 6762, multicast DNS). A push attempt against
/// this name resolves through mDNS rather than DNS — so an accidental
/// `ommx push <anonymous>` cannot leak the artifact to a real remote
/// registry; absent an mDNS responder, the push just fails locally.
pub const ANONYMOUS_ARTIFACT_REF_NAME_SUFFIX: &str = ".ommx.local/anonymous";

/// Number of hex chars from the full registry-id used as the
/// hostname prefix. The metadata column stores the full 32-hex UUID;
/// only the hostname rendering truncates. Picked so the prefix is
/// short enough to read at a glance but wide enough (2^32) to avoid
/// realistic collisions across a single user's registries.
const ANONYMOUS_REGISTRY_ID_HOST_LEN: usize = 8;

/// Generate a synthetic [`ocipkg::ImageName`] for an anonymous
/// artifact. Build the image name from the registry's persisted
/// `registry_id` (so the prefix matches the destination registry,
/// not the default registry), plus a local-time timestamp tag.
///
/// Format: `<registry-id8>.ommx.local/anonymous:<local-time>` where
/// - `<registry-id8>` is the first
///   [`ANONYMOUS_REGISTRY_ID_HOST_LEN`] hex chars of the registry's
///   `registry_id` metadata (a random UUID generated once per
///   [`LocalRegistry`] on first init). The full UUID stays on disk
///   for future widening; only the rendering truncates.
/// - `<local-time>` is `YYYYMMDDTHHMMSS` in the caller's **local**
///   time zone (no `Z` / no offset — OCI tag syntax forbids `+`, and
///   a fixed UTC marker would defeat the "I can read the date at a
///   glance" intent). A registry shared across machines in different
///   timezones loses the time component's absolute meaning, which is
///   an explicit non-goal for anonymous artifacts.
pub(crate) fn anonymous_artifact_image_name(registry_id: &str) -> Result<ocipkg::ImageName> {
    let prefix: String = registry_id
        .chars()
        .take(ANONYMOUS_REGISTRY_ID_HOST_LEN)
        .collect();
    anyhow::ensure!(
        prefix.len() == ANONYMOUS_REGISTRY_ID_HOST_LEN
            && prefix
                .chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
        "Anonymous artifact registry id must be at least {} lowercase hex chars; got {prefix:?}",
        ANONYMOUS_REGISTRY_ID_HOST_LEN,
    );
    let stamp = chrono::Local::now().format("%Y%m%dT%H%M%S");
    ocipkg::ImageName::parse(&format!("{prefix}.ommx.local/anonymous:{stamp}"))
        .with_context(|| format!("Failed to synthesise anonymous artifact image name: {prefix}"))
}

/// True iff `name` (the `host/path` portion of an OCI ref) matches the
/// shape an anonymous artifact's image name would take: an 8-hex
/// `<host>.ommx.local/anonymous` repository. Used by
/// `prune-anonymous` to filter SQLite refs without false-positives on
/// human-pushed refs that happen to share the suffix
/// (e.g. an artifact pushed to a real mDNS host named
/// `myhost.ommx.local`).
pub fn is_anonymous_artifact_ref_name(name: &str) -> bool {
    let Some(host_segment) = name.strip_suffix(ANONYMOUS_ARTIFACT_REF_NAME_SUFFIX) else {
        return false;
    };
    host_segment.len() == ANONYMOUS_REGISTRY_ID_HOST_LEN
        && host_segment
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

/// True iff `tag` matches the `YYYYMMDDTHHMMSS` shape that
/// [`anonymous_artifact_image_name`] generates: 15 chars, digits with
/// `T` at position 8. Combined with [`is_anonymous_artifact_ref_name`]
/// this gives `prune-anonymous` a structural match instead of a
/// substring match.
pub fn is_anonymous_artifact_tag(tag: &str) -> bool {
    tag.len() == 15
        && tag
            .chars()
            .enumerate()
            .all(|(i, c)| if i == 8 { c == 'T' } else { c.is_ascii_digit() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_image_name(tag: &str) -> Result<ocipkg::ImageName> {
        ocipkg::ImageName::parse(&format!("ghcr.io/jij-inc/ommx/demo:{tag}"))
    }

    /// `<registry-id8>.ommx.local/anonymous:<YYYYMMDDTHHMMSS>` must
    /// parse as a valid OCI image reference. A regression that
    /// included `:` / `+` in the timestamp, or non-alphanumeric chars
    /// in the registry-id prefix, would break `ImageName::parse`.
    #[test]
    fn anonymous_image_name_parses() {
        let fake_registry_id = "deadbeef0123456789abcdef01234567";
        let name = anonymous_artifact_image_name(fake_registry_id).expect("synthetic ref parses");
        let s = name.to_string();
        // Repository portion ends with `.ommx.local/anonymous`; the
        // tag follows the colon. e.g. `deadbeef.ommx.local/anonymous:20260512T153045`.
        let (before_colon, tag) = s.rsplit_once(':').expect("ref must include `:tag`");
        assert!(
            before_colon.ends_with(ANONYMOUS_ARTIFACT_REF_NAME_SUFFIX),
            "ref `{before_colon}` must end with `{ANONYMOUS_ARTIFACT_REF_NAME_SUFFIX}`",
        );
        assert!(
            before_colon.starts_with("deadbeef."),
            "ref `{before_colon}` must start with the truncated registry-id prefix",
        );
        // Tag is `YYYYMMDDTHHMMSS` (15 chars), no separators except
        // the alphabetic `T` between date and time.
        assert_eq!(tag.len(), 15, "tag `{tag}` must be 15 chars");
        assert!(
            tag.chars().nth(8) == Some('T'),
            "tag `{tag}` must have `T` at position 8",
        );
    }

    /// `is_anonymous_artifact_ref_name` + `is_anonymous_artifact_tag`
    /// must together accept only ref / tag pairs that
    /// [`anonymous_artifact_image_name`] would generate, and reject
    /// substring-match false positives a naive `ends_with` would let
    /// through (the failure mode `ommx artifact prune-anonymous`
    /// would otherwise have on a human-pushed
    /// `myhost.ommx.local/anonymous:v1`).
    #[test]
    fn anonymous_ref_filter_rejects_false_positives() {
        // Positive: a synthesized name + tag pair.
        let synth = anonymous_artifact_image_name("0123456789abcdef0123456789abcdef")
            .unwrap()
            .to_string();
        let (name, tag) = synth.rsplit_once(':').unwrap();
        assert!(is_anonymous_artifact_ref_name(name));
        assert!(is_anonymous_artifact_tag(tag));

        // Negative: hostname has the suffix but a non-8-hex prefix.
        assert!(!is_anonymous_artifact_ref_name(
            "myhost.ommx.local/anonymous"
        ));
        assert!(!is_anonymous_artifact_ref_name(
            "ABCDEFGH.ommx.local/anonymous"
        ));

        // Negative: tag is not the synthesized YYYYMMDDTHHMMSS shape.
        assert!(!is_anonymous_artifact_tag("v1"));
        assert!(!is_anonymous_artifact_tag("20260512-153045"));
        assert!(!is_anonymous_artifact_tag("2026051215304500"));
        assert!(!is_anonymous_artifact_tag("XXXXXXXXTXXXXXX"));
    }

    /// Hostname prefix is truncated to the configured length even when
    /// the persisted registry-id is the full 32-hex UUID, so the
    /// rendered image name stays compact while the metadata column
    /// keeps the full identifier for future use.
    #[test]
    fn anonymous_image_name_truncates_registry_id() {
        let full = "0123456789abcdef0123456789abcdef";
        let name = anonymous_artifact_image_name(full).unwrap().to_string();
        let host = name
            .split('/')
            .next()
            .expect("synthetic ref has a host segment");
        // `<8-hex>.ommx.local` → host segment is the 8-hex + `.ommx.local`.
        assert_eq!(host, format!("{}.ommx.local", &full[..8]));
    }

    /// Two anonymous builds with no sleep between them collide on the
    /// `YYYYMMDDTHHMMSS` tag. The builder transparently overrides
    /// `RefConflictPolicy::Replace` for the anonymous case so the
    /// second build succeeds and silently overwrites the first. A
    /// regression that left the policy at `KeepExisting` would surface
    /// here as the second `build_in_registry` returning an `Err`
    /// describing a ref conflict.
    #[test]
    fn anonymous_build_in_same_second_does_not_fail() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let registry = Arc::new(LocalRegistry::open(dir.path())?);
        for tag in ["a", "b"] {
            let mut builder = LocalArtifactBuilder::new_anonymous();
            builder.add_layer_bytes(
                MediaType::Other(media_types::V1_INSTANCE_MEDIA_TYPE.to_string()),
                format!("anon-{tag}").into_bytes(),
                HashMap::new(),
            )?;
            // Pass `KeepExisting` explicitly: the builder must still
            // override to `Replace` internally for the anonymous case.
            builder.build_in_registry(registry.clone(), RefConflictPolicy::KeepExisting)?;
        }
        Ok(())
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
        // Manifest's own `mediaType` field is intentionally not set, matching
        // the v2 / `ArchiveArtifactBuilder` shape; the OCI Distribution
        // Content-Type header is supplied separately at push time.
        assert_eq!(staged.manifest.media_type().as_ref(), None);
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
