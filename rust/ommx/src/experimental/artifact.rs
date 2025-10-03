//! Experimental Artifact API - Unified format handling
//!
//! This module provides a new Artifact enum that dynamically manages different storage formats:
//! - OCI Archive format (`.ommx` files, default for new artifacts)
//! - OCI Directory format (legacy support)
//! - Remote registry references
//!
//! # Design Goals
//!
//! - Replace the parametric `Artifact<T: Image>` with a simpler enum-based API
//! - Automatic format detection and conversion
//! - Consistent behavior across all storage formats
//!
//! # Status
//!
//! This API is experimental and subject to change. It will eventually replace
//! the current `ommx::artifact::Artifact<T>` implementation.

use crate::artifact::{
    media_types, Config, InstanceAnnotations, ParametricInstanceAnnotations, SampleSetAnnotations,
    SolutionAnnotations,
};
use crate::v1;
use anyhow::{bail, ensure, Context, Result};
use ocipkg::{
    distribution::MediaType,
    image::{Image, OciArchive, OciArtifact, OciDir, Remote},
    oci_spec::image::{Descriptor, ImageManifest},
    Digest, ImageName,
};
use prost::Message as _;
use std::path::Path;

/// OMMX Artifact with dynamic format handling
///
/// This enum replaces the parametric `Artifact<T: Image>` with a simpler API that
/// automatically manages different storage formats.
///
/// # Variants
///
/// - `Archive`: OCI archive format (`.ommx` file, default for new artifacts)
/// - `Dir`: OCI directory format (legacy support)
/// - `Remote`: Remote registry reference (transitions to Archive/Dir after pull)
pub enum Artifact {
    Archive(OciArtifact<OciArchive>),
    Dir(OciArtifact<OciDir>),
    Remote(OciArtifact<Remote>),
}

impl Artifact {
    /// Create an Artifact from an OCI archive file (`.ommx`)
    pub fn from_oci_archive(path: &Path) -> Result<Self> {
        let mut artifact = OciArtifact::from_oci_archive(path)?;
        Self::validate_artifact_type(&mut artifact)?;
        Ok(Self::Archive(artifact))
    }

    /// Create an Artifact from an OCI directory
    pub fn from_oci_dir(path: &Path) -> Result<Self> {
        let mut artifact = OciArtifact::from_oci_dir(path)?;
        Self::validate_artifact_type(&mut artifact)?;
        Ok(Self::Dir(artifact))
    }

    /// Create an Artifact from a remote registry
    pub fn from_remote(image_name: ImageName) -> Result<Self> {
        let artifact = OciArtifact::from_remote(image_name)?;
        Ok(Self::Remote(artifact))
    }

    /// Validate that the artifact has the correct OMMX artifact type
    fn validate_artifact_type<T: Image>(artifact: &mut OciArtifact<T>) -> Result<()> {
        let manifest = artifact.get_manifest()?;
        let ty = manifest
            .artifact_type()
            .as_ref()
            .context("Not an OMMX Artifact")?;
        ensure!(
            *ty == media_types::v1_artifact(),
            "Not an OMMX Artifact: {}",
            ty
        );
        Ok(())
    }

    /// Get the image name if available
    pub fn image_name(&mut self) -> Option<String> {
        match self {
            Self::Archive(a) => a.get_name().ok().map(|n| n.to_string()),
            Self::Dir(a) => a.get_name().ok().map(|n| n.to_string()),
            Self::Remote(a) => a.get_name().ok().map(|n| n.to_string()),
        }
    }

    /// Get the manifest
    pub fn get_manifest(&mut self) -> Result<ImageManifest> {
        match self {
            Self::Archive(a) => {
                let manifest = a.get_manifest()?;
                Self::validate_manifest(&manifest)?;
                Ok(manifest)
            }
            Self::Dir(a) => {
                let manifest = a.get_manifest()?;
                Self::validate_manifest(&manifest)?;
                Ok(manifest)
            }
            Self::Remote(a) => {
                let manifest = a.get_manifest()?;
                Self::validate_manifest(&manifest)?;
                Ok(manifest)
            }
        }
    }

    fn validate_manifest(manifest: &ImageManifest) -> Result<()> {
        let ty = manifest
            .artifact_type()
            .as_ref()
            .context("Not an OMMX Artifact")?;
        ensure!(
            *ty == media_types::v1_artifact(),
            "Not an OMMX Artifact: {}",
            ty
        );
        Ok(())
    }

    /// Get layer descriptors filtered by media type
    pub fn get_layer_descriptors(&mut self, media_type: &MediaType) -> Result<Vec<Descriptor>> {
        let manifest = self.get_manifest()?;
        Ok(manifest
            .layers()
            .iter()
            .filter(|desc| desc.media_type() == media_type)
            .cloned()
            .collect())
    }

    /// Get a specific layer by digest
    pub fn get_layer(&mut self, digest: &Digest) -> Result<(Descriptor, Vec<u8>)> {
        let layers = match self {
            Self::Archive(a) => a.get_layers()?,
            Self::Dir(a) => a.get_layers()?,
            Self::Remote(a) => a.get_layers()?,
        };
        for (desc, blob) in layers {
            if desc.digest() == &digest.to_string() {
                return Ok((desc, blob));
            }
        }
        bail!("Layer of digest {} not found", digest)
    }

    /// Get blob by digest
    pub fn get_blob(&mut self, digest: &Digest) -> Result<Vec<u8>> {
        match self {
            Self::Archive(a) => a.get_blob(digest),
            Self::Dir(a) => a.get_blob(digest),
            Self::Remote(a) => a.get_blob(digest),
        }
    }

    /// Get the config
    pub fn get_config(&mut self) -> Result<Config> {
        let (_desc, blob) = match self {
            Self::Archive(a) => a.get_config()?,
            Self::Dir(a) => a.get_config()?,
            Self::Remote(a) => a.get_config()?,
        };
        let config = serde_json::from_slice(&blob)?;
        Ok(config)
    }

    /// Get a solution by digest
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

    /// Get a sample set by digest
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

    /// Get an instance by digest
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

    /// Get a parametric instance by digest
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

    /// Get all solutions
    pub fn get_solutions(&mut self) -> Result<Vec<(Descriptor, v1::State)>> {
        let mut out = Vec::new();
        let layers = match self {
            Self::Archive(a) => a.get_layers()?,
            Self::Dir(a) => a.get_layers()?,
            Self::Remote(a) => a.get_layers()?,
        };
        for (desc, blob) in layers {
            if desc.media_type() != &media_types::v1_solution() {
                continue;
            }
            let solution = v1::State::decode(blob.as_slice())?;
            out.push((desc, solution));
        }
        Ok(out)
    }

    /// Get all instances
    pub fn get_instances(&mut self) -> Result<Vec<(Descriptor, v1::Instance)>> {
        let mut out = Vec::new();
        let layers = match self {
            Self::Archive(a) => a.get_layers()?,
            Self::Dir(a) => a.get_layers()?,
            Self::Remote(a) => a.get_layers()?,
        };
        for (desc, blob) in layers {
            if desc.media_type() != &media_types::v1_instance() {
                continue;
            }
            let instance = v1::Instance::decode(blob.as_slice())?;
            out.push((desc, instance));
        }
        Ok(out)
    }
}
