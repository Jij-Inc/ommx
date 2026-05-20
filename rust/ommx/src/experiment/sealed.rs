//! Read-only model reconstructed from a sealed Experiment Artifact.

use super::parameter::{RunParameterCell, RunParameterTable};
use super::{
    SealedExperiment, ANN_ARTIFACT_KIND, ANN_EXPERIMENT_SCHEMA, ANN_LAYER, ANN_RECORD_NAME,
    ANN_RUN_ID, ANN_SPACE, ARTIFACT_KIND_EXPERIMENT, EXPERIMENT_SCHEMA_V1,
    LAYER_KIND_RUN_PARAMETERS, RUN_PARAMETERS_MEDIA_TYPE,
};
use crate::artifact::{ImageRef, LocalArtifact};
use anyhow::{Context, Result};
use oci_spec::image::{Descriptor, MediaType};
use std::collections::BTreeSet;

impl<'reg> SealedExperiment<'reg> {
    /// Reconstruct a sealed Experiment from a committed Experiment Artifact.
    pub fn from_artifact(artifact: LocalArtifact<'reg>) -> Result<Self> {
        validate_experiment_profile(&artifact)?;
        let layers = artifact.layers()?;
        let mut records = Vec::new();
        let mut record_keys = BTreeSet::new();
        let mut run_parameters = None;

        for layer in layers {
            if is_run_parameter_layer(&layer) {
                if run_parameters.is_some() {
                    crate::bail!("Experiment Artifact contains multiple run-parameter layers");
                }
                let bytes = artifact.get_blob(layer.digest())?;
                run_parameters = Some(
                    serde_json::from_slice::<RunParameterTable>(&bytes)
                        .context("Failed to decode run-parameter table JSON")?,
                );
                continue;
            }

            let Some(record) = ExperimentRecord::from_descriptor(layer)? else {
                continue;
            };
            let key = record.key();
            if !record_keys.insert(key) {
                crate::bail!(
                    "Experiment Artifact contains duplicate Record: space={}, run_id={:?}, \
                     media_type={}, name={}",
                    record.space.as_str(),
                    record.run_id,
                    record.media_type,
                    record.name,
                );
            }
            records.push(record);
        }

        Ok(Self {
            artifact,
            records,
            run_parameters: run_parameters.unwrap_or_else(RunParameterTable::empty),
        })
    }

    pub fn image_name(&self) -> &ImageRef {
        self.artifact.image_name()
    }

    pub fn records(&self) -> &[ExperimentRecord] {
        &self.records
    }

    pub fn run_parameter_cells(&self) -> Vec<RunParameterCell> {
        self.run_parameters.cells()
    }
}

/// Record descriptor visible through a sealed Experiment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExperimentRecord {
    pub space: ExperimentRecordSpace,
    pub run_id: Option<u64>,
    pub name: String,
    pub media_type: String,
    pub descriptor: Descriptor,
}

impl ExperimentRecord {
    fn from_descriptor(descriptor: Descriptor) -> Result<Option<Self>> {
        let annotations = descriptor.annotations().as_ref();
        let Some(space) = annotation(annotations, ANN_SPACE) else {
            return Ok(None);
        };
        let space = ExperimentRecordSpace::parse(space)?;
        let run_id = match space {
            ExperimentRecordSpace::Experiment => {
                if annotation(annotations, ANN_RUN_ID).is_some() {
                    crate::bail!("Experiment-space Record must not have `{ANN_RUN_ID}`");
                }
                None
            }
            ExperimentRecordSpace::Run => Some(
                annotation(annotations, ANN_RUN_ID)
                    .with_context(|| format!("Run-space Record is missing `{ANN_RUN_ID}`"))?
                    .parse::<u64>()
                    .with_context(|| format!("Invalid `{ANN_RUN_ID}` annotation"))?,
            ),
        };
        let name = annotation(annotations, ANN_RECORD_NAME)
            .with_context(|| format!("Experiment Record is missing `{ANN_RECORD_NAME}`"))?
            .to_string();
        let media_type = media_type_to_string(descriptor.media_type());
        Ok(Some(Self {
            space,
            run_id,
            name,
            media_type,
            descriptor,
        }))
    }

    fn key(&self) -> (ExperimentRecordSpace, Option<u64>, String, String) {
        (
            self.space,
            self.run_id,
            self.media_type.clone(),
            self.name.clone(),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExperimentRecordSpace {
    Experiment,
    Run,
}

impl ExperimentRecordSpace {
    fn parse(value: &str) -> Result<Self> {
        match value {
            "experiment" => Ok(Self::Experiment),
            "run" => Ok(Self::Run),
            other => crate::bail!("Unknown `{ANN_SPACE}` annotation value: {other}"),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Experiment => "experiment",
            Self::Run => "run",
        }
    }
}

fn validate_experiment_profile(artifact: &LocalArtifact<'_>) -> Result<()> {
    let annotations = artifact.annotations()?;
    let kind = annotations
        .get(ANN_ARTIFACT_KIND)
        .with_context(|| format!("Artifact is missing `{ANN_ARTIFACT_KIND}` annotation"))?;
    if kind != ARTIFACT_KIND_EXPERIMENT {
        crate::bail!("Artifact kind is `{kind}`, expected `{ARTIFACT_KIND_EXPERIMENT}`");
    }
    let schema = annotations
        .get(ANN_EXPERIMENT_SCHEMA)
        .with_context(|| format!("Experiment Artifact is missing `{ANN_EXPERIMENT_SCHEMA}`"))?;
    if schema != EXPERIMENT_SCHEMA_V1 {
        crate::bail!("Unsupported Experiment schema `{schema}`");
    }
    Ok(())
}

fn is_run_parameter_layer(descriptor: &Descriptor) -> bool {
    descriptor.media_type() == &MediaType::Other(RUN_PARAMETERS_MEDIA_TYPE.to_string())
        && descriptor
            .annotations()
            .as_ref()
            .and_then(|annotations| annotations.get(ANN_LAYER))
            .is_some_and(|layer| layer == LAYER_KIND_RUN_PARAMETERS)
}

fn annotation<'a>(
    annotations: Option<&'a std::collections::HashMap<String, String>>,
    key: &str,
) -> Option<&'a str> {
    annotations.and_then(|annotations| annotations.get(key).map(String::as_str))
}

fn media_type_to_string(media_type: &MediaType) -> String {
    match media_type {
        MediaType::Other(value) => value.clone(),
        other => other.to_string(),
    }
}
