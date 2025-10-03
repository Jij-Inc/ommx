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

mod builder;
mod io;
mod layers;
#[cfg(test)]
mod tests;

pub use builder::Builder;

use crate::artifact::media_types;
use anyhow::{ensure, Context, Result};
use ocipkg::{
    image::{Image, OciArchive, OciArtifact, OciDir, Remote},
    oci_spec::image::Descriptor,
    ImageName,
};
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

    /// Get the image name if available
    pub fn image_name(&mut self) -> Option<String> {
        match self {
            Self::Archive(a) => a.get_name().ok().map(|n| n.to_string()),
            Self::Dir(a) => a.get_name().ok().map(|n| n.to_string()),
            Self::Remote(a) => a.get_name().ok().map(|n| n.to_string()),
        }
    }

    /// Get manifest annotations
    pub fn annotations(&mut self) -> Result<std::collections::HashMap<String, String>> {
        let manifest = self.get_manifest()?;
        Ok(manifest.annotations().clone().unwrap_or_default())
    }

    /// Get layer descriptors
    pub fn layers(&mut self) -> Result<Vec<Descriptor>> {
        let manifest = self.get_manifest()?;
        Ok(manifest.layers().to_vec())
    }

    /// Validate that the artifact has the correct OMMX artifact type
    pub(crate) fn validate_artifact_type<T: Image>(artifact: &mut OciArtifact<T>) -> Result<()> {
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
}
