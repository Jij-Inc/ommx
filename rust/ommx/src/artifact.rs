//! Manage messages as container
//!

mod media_type;
pub use media_type::*;

use anyhow::{Context, Result};
use ocipkg::{
    image::{
        Image, ImageBuilder, OciArchiveBuilder, OciArtifact, OciArtifactBuilder, OciDirBuilder,
    },
    ImageName,
};
use std::{
    ops::{Deref, DerefMut},
    path::PathBuf,
};

/// Root directory for OMMX artifacts
pub fn data_dir() -> Result<PathBuf> {
    Ok(directories::ProjectDirs::from("org", "ommx", "ommx")
        .context("Failed to get project directories")?
        .data_dir()
        .to_path_buf())
}

/// OCI Artifact of artifact type [`application/org.ommx.v1.artifact`][v1_artifact]
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

/// Build [Artifact]
pub struct Builder<Base: ImageBuilder>(OciArtifactBuilder<Base>);

impl Builder<OciArchiveBuilder> {
    pub fn new_archive_unnamed(path: PathBuf) -> Result<Self> {
        let archive = OciArchiveBuilder::new_unnamed(path)?;
        Ok(Self(OciArtifactBuilder::new(
            archive,
            media_type::v1_artifact(),
        )?))
    }

    pub fn new_archive(path: PathBuf, image_name: ImageName) -> Result<Self> {
        let archive = OciArchiveBuilder::new(path, image_name)?;
        Ok(Self(OciArtifactBuilder::new(
            archive,
            media_type::v1_artifact(),
        )?))
    }
}

impl Builder<OciDirBuilder> {
    pub fn new(image_name: ImageName) -> Result<Self> {
        let dir = data_dir()?.join(image_name.as_path());
        let layout = OciDirBuilder::new(dir, image_name)?;
        Ok(Self(OciArtifactBuilder::new(
            layout,
            media_type::v1_artifact(),
        )?))
    }
}

impl<Base: ImageBuilder> Builder<Base> {
    pub fn build(self) -> Result<Artifact<Base::Image>> {
        Ok(Artifact(self.0.build()?))
    }
}
