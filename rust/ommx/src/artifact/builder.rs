use crate::{
    artifact::{data_dir, media_type, Artifact, Config, InstanceAnnotations, SolutionAnnotations},
    v1,
};
use anyhow::Result;
use ocipkg::{
    image::{ImageBuilder, OciArchiveBuilder, OciArtifactBuilder, OciDirBuilder},
    ImageName,
};
use prost::Message;
use std::{collections::HashMap, path::PathBuf};

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
        Ok(Artifact::new(self.0.build()?)?)
    }
}
