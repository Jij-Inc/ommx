//! Read-only model reconstructed from a sealed Experiment Artifact.

use super::config::ExperimentConfig;
use super::parameter::{RunParameterCell, RunParameterTable};
use super::{
    SealedExperiment, ANN_ARTIFACT_KIND, ANN_EXPERIMENT_SCHEMA, ANN_RECORD_NAME,
    ARTIFACT_KIND_EXPERIMENT, EXPERIMENT_CONFIG_MEDIA_TYPE, EXPERIMENT_SCHEMA_V1,
    RUN_PARAMETERS_MEDIA_TYPE,
};
use crate::artifact::{ImageRef, LocalArtifact};
use anyhow::{Context, Result};
use oci_spec::image::{Descriptor, MediaType};
use std::collections::{BTreeMap, BTreeSet};

impl<'reg> SealedExperiment<'reg> {
    /// Reconstruct a sealed Experiment from a committed Experiment Artifact.
    pub fn from_artifact(artifact: LocalArtifact<'reg>) -> Result<Self> {
        validate_experiment_profile(&artifact)?;
        let config = load_experiment_config(&artifact)?;
        validate_experiment_schema(&config.schema)?;

        let records = decode_records(config.records, "experiment")?;
        let mut runs = BTreeMap::new();
        for run in config.runs {
            let records = decode_records(run.records, &format!("run {}", run.run_id))?;
            if runs
                .insert(
                    run.run_id,
                    SealedRun {
                        run_id: run.run_id,
                        records,
                    },
                )
                .is_some()
            {
                crate::bail!("Experiment config contains duplicate Run {}", run.run_id);
            }
        }
        let run_parameters = load_run_parameters(&artifact, &config.run_parameters)?;
        validate_run_parameters_reference_config_runs(&run_parameters, &runs)?;

        Ok(Self {
            artifact,
            records,
            runs,
            run_parameters,
        })
    }

    pub fn image_name(&self) -> &ImageRef {
        self.artifact.image_name()
    }

    pub fn experiment_records(&self) -> &[ExperimentRecord] {
        &self.records
    }

    pub fn runs(&self) -> impl Iterator<Item = &SealedRun> {
        self.runs.values()
    }

    pub fn run(&self, run_id: u64) -> Option<&SealedRun> {
        self.runs.get(&run_id)
    }

    pub fn run_parameter_cells(&self) -> Vec<RunParameterCell> {
        self.run_parameters.cells()
    }
}

/// Read-only Run reconstructed from a sealed Experiment config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedRun {
    run_id: u64,
    records: Vec<ExperimentRecord>,
}

impl SealedRun {
    pub fn run_id(&self) -> u64 {
        self.run_id
    }

    pub fn records(&self) -> &[ExperimentRecord] {
        &self.records
    }
}

/// Record descriptor visible through a sealed Experiment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExperimentRecord {
    name: String,
    descriptor: Descriptor,
}

impl ExperimentRecord {
    fn from_descriptor(descriptor: Descriptor) -> Result<Self> {
        let name = descriptor
            .annotations()
            .as_ref()
            .and_then(|annotations| annotations.get(ANN_RECORD_NAME))
            .with_context(|| format!("Experiment Record is missing `{ANN_RECORD_NAME}`"))?
            .to_string();
        Ok(Self { name, descriptor })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn media_type(&self) -> String {
        media_type_to_string(self.descriptor.media_type())
    }

    pub fn descriptor(&self) -> &Descriptor {
        &self.descriptor
    }

    fn key(&self) -> (String, String) {
        (self.media_type(), self.name.clone())
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

fn load_experiment_config(artifact: &LocalArtifact<'_>) -> Result<ExperimentConfig> {
    let config = artifact.get_manifest()?.config();
    if config.media_type() != &MediaType::Other(EXPERIMENT_CONFIG_MEDIA_TYPE.to_string()) {
        crate::bail!(
            "Experiment config media type is {}, expected {}",
            media_type_to_string(config.media_type()),
            EXPERIMENT_CONFIG_MEDIA_TYPE
        );
    }
    let bytes = artifact.get_blob(config.digest())?;
    serde_json::from_slice::<ExperimentConfig>(&bytes).context("Failed to decode Experiment config")
}

fn validate_experiment_schema(schema: &str) -> Result<()> {
    if schema != EXPERIMENT_SCHEMA_V1 {
        crate::bail!("Unsupported Experiment config schema `{schema}`");
    }
    Ok(())
}

fn decode_records(records: Vec<Descriptor>, owner: &str) -> Result<Vec<ExperimentRecord>> {
    let mut decoded = Vec::new();
    let mut keys = BTreeSet::new();
    for descriptor in records {
        let record = ExperimentRecord::from_descriptor(descriptor)
            .with_context(|| format!("Failed to decode Record in {owner}"))?;
        let key = record.key();
        if !keys.insert(key) {
            crate::bail!(
                "Experiment config contains duplicate Record in {owner}: media_type={}, name={}",
                record.media_type(),
                record.name(),
            );
        }
        decoded.push(record);
    }
    Ok(decoded)
}

fn load_run_parameters(
    artifact: &LocalArtifact<'_>,
    descriptor: &Descriptor,
) -> Result<RunParameterTable> {
    if descriptor.media_type() != &MediaType::Other(RUN_PARAMETERS_MEDIA_TYPE.to_string()) {
        crate::bail!(
            "Run-parameter descriptor media type is {}, expected {}",
            media_type_to_string(descriptor.media_type()),
            RUN_PARAMETERS_MEDIA_TYPE
        );
    }
    let bytes = artifact.get_blob(descriptor.digest())?;
    serde_json::from_slice::<RunParameterTable>(&bytes)
        .context("Failed to decode run-parameter table JSON")
}

fn validate_run_parameters_reference_config_runs(
    run_parameters: &RunParameterTable,
    runs: &BTreeMap<u64, SealedRun>,
) -> Result<()> {
    for cell in run_parameters.cells() {
        if !runs.contains_key(&cell.run_id) {
            crate::bail!(
                "Run-parameter table references Run {}, but Experiment config does not contain it",
                cell.run_id
            );
        }
    }
    Ok(())
}

fn media_type_to_string(media_type: &MediaType) -> String {
    match media_type {
        MediaType::Other(value) => value.clone(),
        other => other.to_string(),
    }
}
