//! Manage messages as container
//!

mod annotations;
mod builder;
mod config;
pub mod media_types;
pub use annotations::*;
pub use builder::*;
pub use config::*;

use crate::v1;
use anyhow::{bail, ensure, Context, Result};
use ocipkg::{
    distribution::MediaType,
    image::{
        Image, OciArchive, OciArchiveBuilder, OciArtifact, OciDir, OciDirBuilder, Remote,
        RemoteBuilder,
    },
    oci_spec::image::{Descriptor, ImageManifest},
    Digest, ImageName,
};
use prost::Message;
use std::{env, path::PathBuf};
use std::{
    ops::{Deref, DerefMut},
    path::Path,
};

/// Root directory for OMMX local registry
/// 
/// Uses `OMMX_LOCAL_REGISTRY_ROOT` environment variable if set,
/// otherwise uses the default project data directory
pub fn data_dir() -> Result<PathBuf> {
    if let Ok(custom_dir) = env::var("OMMX_LOCAL_REGISTRY_ROOT") {
        let path = PathBuf::from(custom_dir);
        if !path.exists() {
            std::fs::create_dir_all(&path)
                .with_context(|| format!("Failed to create local registry directory: {}", path.display()))?;
        }
        return Ok(path);
    }
    
    Ok(directories::ProjectDirs::from("org", "ommx", "ommx")
        .context("Failed to get project directories")?
        .data_dir()
        .to_path_buf())
}

pub fn image_dir(image_name: &ImageName) -> Result<PathBuf> {
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

fn gather_oci_dirs(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut images = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if path.join("oci-layout").exists() {
                images.push(path);
            } else {
                let mut sub_images = gather_oci_dirs(&path)?;
                images.append(&mut sub_images)
            }
        }
    }
    Ok(images)
}

fn auth_from_env() -> Result<(String, String, String)> {
    if let (Ok(domain), Ok(username), Ok(password)) = (
        env::var("OMMX_BASIC_AUTH_DOMAIN"),
        env::var("OMMX_BASIC_AUTH_USERNAME"),
        env::var("OMMX_BASIC_AUTH_PASSWORD"),
    ) {
        log::info!(
            "Detect OMMX_BASIC_AUTH_DOMAIN, OMMX_BASIC_AUTH_USERNAME, OMMX_BASIC_AUTH_PASSWORD for authentication."
        );
        return Ok((domain, username, password));
    }
    bail!("No authentication information found in environment variables");
}

/// Get all images stored in the local registry
pub fn get_images() -> Result<Vec<ImageName>> {
    let root = data_dir()?;
    let dirs = gather_oci_dirs(&root)?;
    dirs.into_iter()
        .map(|dir| {
            let relative = dir
                .strip_prefix(&root)
                .context("Failed to get relative path")?;
            ImageName::from_path(relative)
        })
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

    pub fn push(&mut self) -> Result<Artifact<Remote>> {
        let name = self.get_name()?;
        log::info!("Pushing: {name}");
        let mut remote = RemoteBuilder::new(name)?;
        if let Ok((domain, username, password)) = auth_from_env() {
            remote.add_basic_auth(&domain, &username, &password);
        }
        let out = ocipkg::image::copy(self.0.deref_mut(), remote)?;
        Ok(Artifact(OciArtifact::new(out)))
    }

    pub fn load(&mut self) -> Result<()> {
        let image_name = self.get_name()?;
        let path = image_dir(&image_name)?;
        if path.exists() {
            log::trace!("Already exists in local registry: {}", path.display());
            return Ok(());
        }
        log::info!("Loading to local registry: {image_name}");
        ocipkg::image::copy(self.0.deref_mut(), OciDirBuilder::new(path, image_name)?)?;
        Ok(())
    }
}

impl Artifact<OciDir> {
    pub fn from_oci_dir(path: &Path) -> Result<Self> {
        let artifact = OciArtifact::from_oci_dir(path)?;
        Self::new(artifact)
    }

    pub fn push(&mut self) -> Result<Artifact<Remote>> {
        let name = self.get_name()?;
        log::info!("Pushing: {name}");
        let mut remote = RemoteBuilder::new(name)?;
        if let Ok((domain, username, password)) = auth_from_env() {
            remote.add_basic_auth(&domain, &username, &password);
        }
        let out = ocipkg::image::copy(self.0.deref_mut(), remote)?;
        Ok(Artifact(OciArtifact::new(out)))
    }

    pub fn save(&mut self, output: &Path) -> Result<()> {
        if output.exists() {
            bail!("Output file already exists: {}", output.display());
        }
        let builder = if let Ok(name) = self.get_name() {
            OciArchiveBuilder::new(output.to_path_buf(), name)?
        } else {
            OciArchiveBuilder::new_unnamed(output.to_path_buf())?
        };
        ocipkg::image::copy(self.0.deref_mut(), builder)?;
        Ok(())
    }
}

impl Artifact<Remote> {
    pub fn from_remote(image_name: ImageName) -> Result<Self> {
        let artifact = OciArtifact::from_remote(image_name)?;
        Self::new(artifact)
    }

    pub fn pull(&mut self) -> Result<Artifact<OciDir>> {
        let image_name = self.get_name()?;
        let path = image_dir(&image_name)?;
        if path.exists() {
            log::trace!("Already exists in local registry: {}", path.display());
            return Ok(Artifact(OciArtifact::from_oci_dir(&path)?));
        }
        log::info!("Pulling to local registry: {image_name}");
        if let Ok((domain, username, password)) = auth_from_env() {
            self.0.add_basic_auth(&domain, &username, &password);
        }
        let out = ocipkg::image::copy(self.0.deref_mut(), OciDirBuilder::new(path, image_name)?)?;
        Ok(Artifact(OciArtifact::new(out)))
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

    pub fn get_layer(&mut self, digest: &Digest) -> Result<(Descriptor, Vec<u8>)> {
        for (desc, blob) in self.0.get_layers()? {
            if desc.digest() == &digest.to_string() {
                return Ok((desc, blob));
            }
        }
        bail!("Layer of digest {} not found", digest)
    }

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
