//! Manage messages as container
//!

mod annotations;
mod builder;
mod config;
pub mod digest;
pub mod local_registry;
mod manifest;
pub mod media_types;
#[cfg(feature = "remote-artifact")]
mod push;
#[cfg(feature = "remote-artifact")]
mod remote_transport;
mod save;
pub use annotations::*;
pub use builder::*;
pub use config::*;
pub use digest::sha256_digest;
pub(crate) use manifest::{stable_json_bytes, StagedArtifactBlob};
pub use manifest::{LocalArtifact, LocalArtifactBuilder, LocalManifest};
pub use media_types::OCI_IMAGE_MANIFEST_MEDIA_TYPE;

use crate::v1;
use anyhow::{bail, ensure, Context, Result};
use ocipkg::{
    image::{Image, OciArchive, OciArtifact, OciDir, OciDirBuilder},
    oci_spec::image::{Descriptor, ImageManifest, MediaType},
    Digest, ImageName,
};

#[cfg(feature = "remote-artifact")]
use crate::artifact::remote_transport::RemoteTransport;
#[cfg(feature = "remote-artifact")]
use oci_client::RegistryOperation;
use prost::Message;
use std::{env, path::PathBuf, sync::OnceLock};
use std::{
    ops::{Deref, DerefMut},
    path::Path,
};

/// Global storage for the local registry root path
static LOCAL_REGISTRY_ROOT: OnceLock<PathBuf> = OnceLock::new();

/// Set the root directory for OMMX local registry
///
/// See [`get_local_registry_root`] for details.
///
pub fn set_local_registry_root(path: impl Into<PathBuf>) -> Result<()> {
    let path = path.into();
    LOCAL_REGISTRY_ROOT.set(path.clone()).map_err(|path| {
        anyhow::anyhow!(
            "Local registry root has already been set: {}",
            path.display()
        )
    })?;
    tracing::info!("Local registry root set via API: {}", path.display());
    Ok(())
}

/// Get the root directory for OMMX local registry
///
/// - Once the root is set, it is immutable for the lifetime of the program.
/// - You can set it via [`set_local_registry_root`] function before calling this.
/// - If this is called without calling [`set_local_registry_root`],
///   - It will check the `OMMX_LOCAL_REGISTRY_ROOT` environment variable.
///   - If the environment variable is not set, it will use the default project data directory.
/// - The root directory is **NOT** created automatically by this function.
///
pub fn get_local_registry_root() -> &'static Path {
    LOCAL_REGISTRY_ROOT.get_or_init(|| {
        // Try environment variable first
        let path = if let Ok(custom_dir) = env::var("OMMX_LOCAL_REGISTRY_ROOT") {
            let path = PathBuf::from(custom_dir);
            tracing::info!(
                "Local registry root initialized from OMMX_LOCAL_REGISTRY_ROOT: {}",
                path.display()
            );
            path
        } else {
            let path = directories::ProjectDirs::from("org", "ommx", "ommx")
                .expect("Failed to get project directories")
                .data_dir()
                .to_path_buf();
            tracing::info!(
                "Local registry root initialized to default: {}",
                path.display()
            );
            path
        };
        path
    })
}

#[deprecated(note = "Use get_local_registry_root instead")]
pub fn data_dir() -> Result<PathBuf> {
    let path = get_local_registry_root().to_path_buf();
    if !path.exists() {
        std::fs::create_dir_all(&path)
            .with_context(|| format!("Failed to create data directory: {}", path.display()))?;
    }
    Ok(path)
}

/// Get the directory for the given image name in the local registry
pub fn get_image_dir(image_name: &ImageName) -> PathBuf {
    get_local_registry_root().join(image_name.as_path())
}

#[deprecated(note = "Use get_image_dir instead")]
pub fn image_dir(image_name: &ImageName) -> Result<PathBuf> {
    #[allow(deprecated)]
    Ok(data_dir()?.join(image_name.as_path()))
}

pub fn ghcr(org: &str, repo: &str, name: &str, tag: &str) -> Result<ImageName> {
    ImageName::parse(&format!(
        "ghcr.io/{}/{}/{}:{}",
        org.to_lowercase(),
        repo.to_lowercase(),
        name.to_lowercase(),
        tag
    ))
}

/// Pull only the manifest for `image_name` from its remote registry,
/// without populating the v3 SQLite Local Registry. Used by CLI
/// `ommx inspect <remote-ref>` so the user can read what is on the
/// other side of a ref without committing to a full pull. For the
/// full pull-into-registry flow use [`local_registry::pull_image`].
///
/// Credentials are resolved by [`remote_transport::RemoteTransport`]'s
/// three-tier chain (env override → `~/.docker/config.json` →
/// anonymous), matching every other network call on the SDK.
#[cfg(feature = "remote-artifact")]
pub fn fetch_remote_manifest(image_name: &ImageName) -> Result<ImageManifest> {
    let transport = RemoteTransport::new(image_name)?;
    transport.auth_for(image_name, RegistryOperation::Pull)?;
    let (manifest_bytes, _digest) =
        transport.pull_manifest_raw(image_name, &[OCI_IMAGE_MANIFEST_MEDIA_TYPE])?;
    serde_json::from_slice(&manifest_bytes)
        .context("Failed to parse OCI image manifest from the remote registry")
}

/// Get all images stored in the local registry
pub fn get_images() -> Result<Vec<ImageName>> {
    let root = get_local_registry_root();
    let registry = local_registry::LocalRegistry::open(root)?;
    registry
        .index()
        .list_refs(None)?
        .into_iter()
        .map(|reference| ImageName::parse(&format!("{}:{}", reference.name, reference.reference)))
        .collect()
}

/// OMMX Artifact, an OCI Artifact of type [`application/org.ommx.v1.artifact`][media_types::v1_artifact]
pub struct Artifact<Base: Image>(OciArtifact<Base>);

impl<Base: Image> Deref for Artifact<Base> {
    type Target = OciArtifact<Base>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<Base: Image> DerefMut for Artifact<Base> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Artifact<OciArchive> {
    pub fn from_oci_archive(path: &Path) -> Result<Self> {
        let artifact = OciArtifact::from_oci_archive(path)?;
        Self::new(artifact)
    }

    /// Push this archive to its OCI registry through the v3 native
    /// transport. Reads manifest + config + every layer blob from
    /// the archive once and uploads each via
    /// [`remote_transport::RemoteTransport`]; the bytes never touch
    /// the v3 SQLite Local Registry, so a push of an archive that
    /// has not been imported is side-effect-free locally.
    ///
    /// Credentials are resolved by the three-tier chain (env →
    /// `~/.docker/config.json` → anonymous). Blobs are pushed before
    /// the manifest so a partial failure leaves the registry without
    /// a tag pointing at incomplete data; OCI cross-blob-mount /
    /// "blob already exists" optimisation is a follow-up refinement.
    #[cfg(feature = "remote-artifact")]
    #[tracing::instrument(skip_all, fields(artifact_storage = "oci_archive"))]
    pub fn push(&mut self) -> Result<()> {
        let image_name = self.get_name()?;
        let manifest = self.get_manifest()?;
        let manifest_digest = sha256_digest_of_manifest(&manifest)?;
        let manifest_bytes = serde_json::to_vec(&manifest)
            .context("Failed to re-serialise OCI image manifest from the archive")?;

        tracing::info!("Pushing {image_name} from archive");
        let transport = RemoteTransport::new(&image_name)?;
        transport.auth_for(&image_name, RegistryOperation::Push)?;

        let descriptors: Vec<Descriptor> = std::iter::once(manifest.config().clone())
            .chain(manifest.layers().iter().cloned())
            .collect();
        for descriptor in &descriptors {
            let digest = descriptor.digest().to_string();
            let parsed_digest: Digest = digest.parse().with_context(|| {
                format!("Invalid blob digest {digest} in manifest for {image_name}")
            })?;
            let bytes = self.0.get_blob(&parsed_digest)?;
            tracing::debug!(size = bytes.len(), "Pushing blob {digest} of {image_name}");
            transport.push_blob(&image_name, &digest, bytes)?;
        }

        let content_type = manifest
            .media_type()
            .as_ref()
            .map(MediaType::to_string)
            .unwrap_or_else(|| OCI_IMAGE_MANIFEST_MEDIA_TYPE.to_string());
        tracing::info!(
            "Publishing manifest {manifest_digest} ({content_type}, {} bytes) to {image_name}",
            manifest_bytes.len(),
        );
        transport.push_manifest_bytes(&image_name, manifest_bytes, &content_type)?;
        Ok(())
    }

    /// Load this archive into an OCI Image Layout at the explicit
    /// `target_path`. Used by the v3 Local Registry import path with
    /// a caller-owned tempdir under the registry root so the staged
    /// layout lives on the same filesystem as the `FileBlobStore`;
    /// the tempdir is dropped once the import has copied the bytes
    /// into the SQLite + `FileBlobStore` registry.
    #[tracing::instrument(skip_all, fields(artifact_storage = "oci_archive", target_path = %target_path.display()))]
    pub fn load_to(&mut self, target_path: &Path) -> Result<()> {
        let image_name = self.get_name()?;
        if target_path.exists() {
            tracing::trace!("Already exists at: {}", target_path.display());
            return Ok(());
        }
        tracing::info!("Loading {image_name} to {}", target_path.display());
        ocipkg::image::copy(
            self.0.deref_mut(),
            OciDirBuilder::new(target_path.to_path_buf(), image_name)?,
        )?;
        Ok(())
    }
}

/// SHA-256 digest of the manifest as we re-serialise it for push. We
/// serialise once and hash that exact byte sequence so the digest
/// reported in tracing matches what `RemoteTransport::push_manifest_bytes`
/// uploads.
#[cfg(feature = "remote-artifact")]
fn sha256_digest_of_manifest(manifest: &ImageManifest) -> Result<String> {
    let bytes = serde_json::to_vec(manifest)
        .context("Failed to serialise OCI image manifest for digest computation")?;
    Ok(sha256_digest(&bytes))
}

impl Artifact<OciDir> {
    /// Open an existing OCI Image Layout directory for read. v3 has
    /// no push / save / load_to surface on `OciDir`: archive / dir
    /// inputs flow through [`local_registry::import_oci_dir`] /
    /// [`local_registry::import_oci_archive`] into the SQLite Local
    /// Registry and are pushed via [`LocalArtifact::push`]. Direct
    /// `OciDir` reads remain available so callers that already hold
    /// an OCI Image Layout (e.g. `oras` exports) can inspect it
    /// without importing.
    pub fn from_oci_dir(path: &Path) -> Result<Self> {
        let artifact = OciArtifact::from_oci_dir(path)?;
        Self::new(artifact)
    }
}

impl<Base: Image> Artifact<Base> {
    pub fn new(artifact: OciArtifact<Base>) -> Result<Self> {
        Ok(Self(artifact))
    }

    pub fn get_manifest(&mut self) -> Result<ImageManifest> {
        let manifest = self.0.get_manifest()?;
        let ty = manifest
            .artifact_type()
            .as_ref()
            .context("Not an OMMX Artifact")?;
        ensure!(
            *ty == media_types::v1_artifact(),
            "Not an OMMX Artifact: {}",
            ty
        );
        Ok(manifest)
    }

    pub fn get_config(&mut self) -> Result<Config> {
        let (_desc, blob) = self.0.get_config()?;
        let config = serde_json::from_slice(&blob)?;
        Ok(config)
    }

    pub fn get_layer_descriptors(&mut self, media_type: &MediaType) -> Result<Vec<Descriptor>> {
        let manifest = self.get_manifest()?;
        Ok(manifest
            .layers()
            .iter()
            .filter(|desc| desc.media_type() == media_type)
            .cloned()
            .collect())
    }

    #[tracing::instrument(skip_all, fields(digest = %digest))]
    pub fn get_layer(&mut self, digest: &Digest) -> Result<(Descriptor, Vec<u8>)> {
        for (desc, blob) in self.0.get_layers()? {
            if desc.digest() == digest {
                return Ok((desc, blob));
            }
        }
        bail!("Layer of digest {} not found", digest)
    }

    #[tracing::instrument(skip_all, fields(digest = %digest))]
    pub fn get_solution(&mut self, digest: &Digest) -> Result<(v1::State, SolutionAnnotations)> {
        let (desc, blob) = self.get_layer(digest)?;
        ensure!(
            desc.media_type() == &media_types::v1_solution(),
            "Layer {digest} is not an ommx.v1.Solution: {}",
            desc.media_type()
        );
        Ok((
            v1::State::decode(blob.as_slice())?,
            SolutionAnnotations::from_descriptor(&desc),
        ))
    }

    #[tracing::instrument(skip_all, fields(digest = %digest))]
    pub fn get_sample_set(
        &mut self,
        digest: &Digest,
    ) -> Result<(v1::SampleSet, SampleSetAnnotations)> {
        let (desc, blob) = self.get_layer(digest)?;
        ensure!(
            desc.media_type() == &media_types::v1_sample_set(),
            "Layer {digest} is not an ommx.v1.SampleSet: {}",
            desc.media_type()
        );
        Ok((
            v1::SampleSet::decode(blob.as_slice())?,
            SampleSetAnnotations::from_descriptor(&desc),
        ))
    }

    #[tracing::instrument(skip_all, fields(digest = %digest))]
    pub fn get_instance(&mut self, digest: &Digest) -> Result<(v1::Instance, InstanceAnnotations)> {
        let (desc, blob) = self.get_layer(digest)?;
        ensure!(
            desc.media_type() == &media_types::v1_instance(),
            "Layer {digest} is not an ommx.v1.Instance: {}",
            desc.media_type()
        );
        Ok((
            v1::Instance::decode(blob.as_slice())?,
            InstanceAnnotations::from_descriptor(&desc),
        ))
    }

    #[tracing::instrument(skip_all, fields(digest = %digest))]
    pub fn get_parametric_instance(
        &mut self,
        digest: &Digest,
    ) -> Result<(v1::ParametricInstance, ParametricInstanceAnnotations)> {
        let (desc, blob) = self.get_layer(digest)?;
        ensure!(
            desc.media_type() == &media_types::v1_parametric_instance(),
            "Layer {digest} is not an ommx.v1.ParametricInstance: {}",
            desc.media_type()
        );
        Ok((
            v1::ParametricInstance::decode(blob.as_slice())?,
            ParametricInstanceAnnotations::from_descriptor(&desc),
        ))
    }

    pub fn get_solutions(&mut self) -> Result<Vec<(Descriptor, v1::State)>> {
        let mut out = Vec::new();
        for (desc, blob) in self.0.get_layers()? {
            if desc.media_type() != &media_types::v1_solution() {
                continue;
            }
            let solution = v1::State::decode(blob.as_slice())?;
            out.push((desc, solution));
        }
        Ok(out)
    }

    pub fn get_instances(&mut self) -> Result<Vec<(Descriptor, v1::Instance)>> {
        let mut out = Vec::new();
        for (desc, blob) in self.0.get_layers()? {
            if desc.media_type() != &media_types::v1_instance() {
                continue;
            }
            let instance = v1::Instance::decode(blob.as_slice())?;
            out.push((desc, instance));
        }
        Ok(out)
    }
}
