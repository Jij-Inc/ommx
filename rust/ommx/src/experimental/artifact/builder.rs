use super::Artifact;
use crate::artifact::{
    get_local_registry_root, media_types, Config, InstanceAnnotations,
    ParametricInstanceAnnotations, SampleSetAnnotations, SolutionAnnotations,
};
use crate::v1;
use anyhow::{bail, Context, Result};
use ocipkg::{
    image::{OciArchiveBuilder, OciArtifactBuilder, OciDirBuilder},
    ImageName,
};
use prost::Message;
use std::{collections::HashMap, path::PathBuf};
use uuid::Uuid;

/// Builder for experimental [`Artifact`] values with runtime-selected backend.
///
/// The builder mirrors the legacy generic builders but stores the backend choice
/// inside the enum variants, allowing callers to construct archives or
/// directories without specifying the type parameter in the signature.
pub enum Builder {
    Archive(OciArtifactBuilder<OciArchiveBuilder>),
    Dir(OciArtifactBuilder<OciDirBuilder>),
}

impl Builder {
    /// Create a builder that writes into an OCI archive file at `path`.
    pub fn new_archive(path: PathBuf, image_name: ImageName) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create parent directory: {}", parent.display())
            })?;
        }
        let archive = OciArchiveBuilder::new(path, image_name)?;
        let builder = OciArtifactBuilder::new(archive, media_types::v1_artifact())?;
        Ok(Self::Archive(builder))
    }

    /// Create an unnamed archive builder (primarily for tests).
    pub fn new_archive_unnamed(path: PathBuf) -> Result<Self> {
        let archive = OciArchiveBuilder::new_unnamed(path)?;
        let builder = OciArtifactBuilder::new(archive, media_types::v1_artifact())?;
        Ok(Self::Archive(builder))
    }

    /// Convenience helper that stores the archive in a temporary location.
    pub fn temp_archive() -> Result<Self> {
        let id = Uuid::new_v4();
        Self::new_archive(
            std::env::temp_dir().join(format!("ommx-{id}")),
            ImageName::parse(&format!("ttl.sh/{id}:1h"))?,
        )
    }

    /// Create a builder that writes to an OCI directory at the provided path.
    pub fn new_dir(path: PathBuf, image_name: ImageName) -> Result<Self> {
        if path.exists() {
            bail!("Output directory already exists: {}", path.display());
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create parent directory: {}", parent.display())
            })?;
        }
        let layout = OciDirBuilder::new(path, image_name)?;
        let builder = OciArtifactBuilder::new(layout, media_types::v1_artifact())?;
        Ok(Self::Dir(builder))
    }

    /// Create a builder targeting the local registry directory for the image name.
    pub fn new_registry_dir(image_name: ImageName) -> Result<Self> {
        let path = get_local_registry_root().join(image_name.as_path());
        Self::new_dir(path, image_name)
    }

    pub fn add_instance(
        &mut self,
        instance: v1::Instance,
        annotations: InstanceAnnotations,
    ) -> Result<()> {
        let blob = instance.encode_to_vec();
        match self {
            Self::Archive(builder) => {
                builder.add_layer(media_types::v1_instance(), &blob, annotations.into())?;
            }
            Self::Dir(builder) => {
                builder.add_layer(media_types::v1_instance(), &blob, annotations.into())?;
            }
        }
        Ok(())
    }

    pub fn add_solution(
        &mut self,
        solution: v1::State,
        annotations: SolutionAnnotations,
    ) -> Result<()> {
        let blob = solution.encode_to_vec();
        match self {
            Self::Archive(builder) => {
                builder.add_layer(media_types::v1_solution(), &blob, annotations.into())?;
            }
            Self::Dir(builder) => {
                builder.add_layer(media_types::v1_solution(), &blob, annotations.into())?;
            }
        }
        Ok(())
    }

    pub fn add_parametric_instance(
        &mut self,
        instance: v1::ParametricInstance,
        annotations: ParametricInstanceAnnotations,
    ) -> Result<()> {
        let blob = instance.encode_to_vec();
        match self {
            Self::Archive(builder) => {
                builder.add_layer(
                    media_types::v1_parametric_instance(),
                    &blob,
                    annotations.into(),
                )?;
            }
            Self::Dir(builder) => {
                builder.add_layer(
                    media_types::v1_parametric_instance(),
                    &blob,
                    annotations.into(),
                )?;
            }
        }
        Ok(())
    }

    pub fn add_sample_set(
        &mut self,
        sample_set: v1::SampleSet,
        annotations: SampleSetAnnotations,
    ) -> Result<()> {
        let blob = sample_set.encode_to_vec();
        match self {
            Self::Archive(builder) => {
                builder.add_layer(media_types::v1_sample_set(), &blob, annotations.into())?;
            }
            Self::Dir(builder) => {
                builder.add_layer(media_types::v1_sample_set(), &blob, annotations.into())?;
            }
        }
        Ok(())
    }

    pub fn add_config(&mut self, config: Config) -> Result<()> {
        let blob = serde_json::to_string_pretty(&config)?;
        match self {
            Self::Archive(builder) => {
                builder.add_config(media_types::v1_config(), blob.as_bytes(), HashMap::new())?;
            }
            Self::Dir(builder) => {
                builder.add_config(media_types::v1_config(), blob.as_bytes(), HashMap::new())?;
            }
        }
        Ok(())
    }

    /// Add an annotation to the manifest
    pub fn add_annotation(&mut self, key: String, value: String) {
        match self {
            Self::Archive(builder) => {
                builder.add_annotation(key, value);
            }
            Self::Dir(builder) => {
                builder.add_annotation(key, value);
            }
        }
    }

    /// Finalise the builder and produce an [`Artifact`] with the same backend variant.
    pub fn build(self) -> Result<Artifact> {
        match self {
            Self::Archive(builder) => {
                let artifact = builder.build()?;
                Ok(Artifact::Archive(artifact))
            }
            Self::Dir(builder) => {
                let artifact = builder.build()?;
                Ok(Artifact::Dir(artifact))
            }
        }
    }
}
