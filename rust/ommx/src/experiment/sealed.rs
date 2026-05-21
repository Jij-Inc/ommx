//! Read-only model reconstructed from a sealed Experiment Artifact.

use super::config::ExperimentConfig;
use super::parameter::{RunParameterCell, RunParameterTable};
use super::record::{media_type_to_string, RecordRef};
use super::{
    SealedExperiment, EXPERIMENT_CONFIG_MEDIA_TYPE, EXPERIMENT_SCHEMA_V1, RUN_PARAMETERS_MEDIA_TYPE,
};
use crate::artifact::{ImageRef, LocalArtifact};
use anyhow::{Context, Result};
use oci_spec::image::{Descriptor, MediaType};
use std::collections::{BTreeMap, BTreeSet};

impl<'reg> SealedExperiment<'reg> {
    /// Reconstruct a sealed Experiment from a committed Experiment Artifact.
    pub fn from_artifact(artifact: LocalArtifact<'reg>) -> Result<Self> {
        let config = load_experiment_config(&artifact)?;
        validate_experiment_schema(&config.schema)?;

        let records = decode_records(artifact.registry(), config.records, "experiment")?;
        let mut runs = BTreeMap::new();
        for run in config.runs {
            let records = decode_records(
                artifact.registry(),
                run.records,
                &format!("run {}", run.run_id),
            )?;
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

    pub fn experiment_records(&self) -> &[RecordRef<'reg>] {
        &self.records
    }

    pub fn runs(&self) -> impl Iterator<Item = &SealedRun<'reg>> {
        self.runs.values()
    }

    pub fn run(&self, run_id: u64) -> Option<&SealedRun<'reg>> {
        self.runs.get(&run_id)
    }

    pub fn run_parameter_cells(&self) -> Vec<RunParameterCell> {
        self.run_parameters.cells()
    }
}

/// Read-only Run reconstructed from a sealed Experiment config.
#[derive(Debug, Clone)]
pub struct SealedRun<'reg> {
    run_id: u64,
    records: Vec<RecordRef<'reg>>,
}

impl<'reg> SealedRun<'reg> {
    pub fn run_id(&self) -> u64 {
        self.run_id
    }

    pub fn records(&self) -> &[RecordRef<'reg>] {
        &self.records
    }
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
        crate::bail!("Unsupported Experiment schema `{schema}`");
    }
    Ok(())
}

fn decode_records<'reg>(
    registry: &'reg crate::artifact::local_registry::LocalRegistry,
    records: Vec<Descriptor>,
    owner: &str,
) -> Result<Vec<RecordRef<'reg>>> {
    let mut decoded = Vec::new();
    let mut keys = BTreeSet::new();
    for descriptor in records {
        let record = RecordRef::from_descriptor(registry, descriptor)
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
    runs: &BTreeMap<u64, SealedRun<'_>>,
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
