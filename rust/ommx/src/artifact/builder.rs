use crate::{
    artifact::{media_types, Artifact, Config, InstanceAnnotations, SolutionAnnotations},
    v1,
};
use anyhow::Result;
use ocipkg::{
    image::{OciArchive, OciArchiveBuilder, OciArtifactBuilder},
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

/// Build an [`Artifact<OciArchive>`] (`.ommx` OCI archive output).
///
/// v3-native build into the SQLite Local Registry uses
/// [`super::LocalArtifactBuilder`] instead; this type is the
/// archive-only legacy path retained for `PyArtifactBuilder.new_archive*`
/// / `temp` and the test suite.
pub struct ArchiveArtifactBuilder(OciArtifactBuilder<OciArchiveBuilder>);

impl Deref for ArchiveArtifactBuilder {
    type Target = OciArtifactBuilder<OciArchiveBuilder>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ArchiveArtifactBuilder {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl ArchiveArtifactBuilder {
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

    pub fn build(self) -> Result<Artifact<OciArchive>> {
        Artifact::new(self.0.build()?)
    }
}
