use crate::{
    artifact::{media_types, Artifact, Config, InstanceAnnotations, SolutionAnnotations},
    v1,
};
use anyhow::Result;
use ocipkg::{
    image::{ImageBuilder, OciArchiveBuilder, OciArtifactBuilder},
    ImageName,
};
use prost::Message;
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    path::PathBuf,
};
use uuid::Uuid;

use super::{ParametricInstanceAnnotations, SampleSetAnnotations};

/// Build [Artifact]
pub struct Builder<Base: ImageBuilder>(OciArtifactBuilder<Base>);

impl<Base: ImageBuilder> Deref for Builder<Base> {
    type Target = OciArtifactBuilder<Base>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<Base: ImageBuilder> DerefMut for Builder<Base> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Builder<OciArchiveBuilder> {
    pub fn new_archive_unnamed(path: PathBuf) -> Result<Self> {
        let archive = OciArchiveBuilder::new_unnamed(path)?;
        Ok(Self(OciArtifactBuilder::new(
            archive,
            media_types::v1_artifact(),
        )?))
    }

    pub fn new_archive(path: PathBuf, image_name: ImageName) -> Result<Self> {
        let archive = OciArchiveBuilder::new(path, image_name)?;
        Ok(Self(OciArtifactBuilder::new(
            archive,
            media_types::v1_artifact(),
        )?))
    }

    /// Create a new artifact builder for a temporary file. This is insecure and should only be used in tests.
    pub fn temp_archive() -> Result<Self> {
        let id = Uuid::new_v4();
        Self::new_archive(
            std::env::temp_dir().join(format!("ommx-{id}")),
            ImageName::parse(&format!("ttl.sh/{id}:1h"))?,
        )
    }
}

impl<Base: ImageBuilder> Builder<Base> {
    pub fn add_instance(
        &mut self,
        instance: v1::Instance,
        annotations: InstanceAnnotations,
    ) -> Result<()> {
        let blob = instance.encode_to_vec();
        self.0
            .add_layer(media_types::v1_instance(), &blob, annotations.into())?;
        Ok(())
    }

    pub fn add_solution(
        &mut self,
        solution: v1::State,
        annotations: SolutionAnnotations,
    ) -> Result<()> {
        let blob = solution.encode_to_vec();
        self.0
            .add_layer(media_types::v1_solution(), &blob, annotations.into())?;
        Ok(())
    }

    pub fn add_parametric_instance(
        &mut self,
        instance: v1::ParametricInstance,
        annotations: ParametricInstanceAnnotations,
    ) -> Result<()> {
        let blob = instance.encode_to_vec();
        self.0.add_layer(
            media_types::v1_parametric_instance(),
            &blob,
            annotations.into(),
        )?;
        Ok(())
    }

    pub fn add_sample_set(
        &mut self,
        sample_set: v1::SampleSet,
        annotations: SampleSetAnnotations,
    ) -> Result<()> {
        let blob = sample_set.encode_to_vec();
        self.0
            .add_layer(media_types::v1_sample_set(), &blob, annotations.into())?;
        Ok(())
    }

    pub fn add_config(&mut self, config: Config) -> Result<()> {
        let blob = serde_json::to_string_pretty(&config)?;
        self.0
            .add_config(media_types::v1_config(), blob.as_bytes(), HashMap::new())?;
        Ok(())
    }

    pub fn build(self) -> Result<Artifact<Base::Image>> {
        Artifact::new(self.0.build()?)
    }
}
