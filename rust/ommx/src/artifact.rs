//! Manage messages as container
//!

mod annotations;
mod config;
mod media_type;
pub use annotations::*;
pub use config::*;
pub use media_type::*;

use anyhow::{Context, Result};
use ocipkg::{
    image::{
        Image, ImageBuilder, OciArchiveBuilder, OciArtifact, OciArtifactBuilder, OciDirBuilder,
    },
    ImageName,
};
use prost::Message;
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    path::PathBuf,
};

use crate::v1;

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
    pub fn add_instance(
        mut self,
        instance: v1::Instance,
        annotations: InstanceAnnotations,
    ) -> Result<Self> {
        let blob = instance.encode_to_vec();
        self.0
            .add_layer(media_type::v1_instance(), &blob, annotations.into())?;
        Ok(self)
    }

    pub fn add_solution(
        mut self,
        solution: v1::Solution,
        annotations: SolutionAnnotations,
    ) -> Result<Self> {
        let blob = solution.encode_to_vec();
        self.0
            .add_layer(media_type::v1_solution(), &blob, annotations.into())?;
        Ok(self)
    }

    pub fn add_config(mut self, config: Config) -> Result<Self> {
        let blob = serde_json::to_string_pretty(&config)?;
        self.0
            .add_config(media_type::v1_config(), blob.as_bytes(), HashMap::new())?;
        Ok(self)
    }

    pub fn build(self) -> Result<Artifact<Base::Image>> {
        Ok(Artifact(self.0.build()?))
    }
}
