//! Read-only model reconstructed from a sealed Experiment Artifact.

use super::config::{ExperimentConfig, ExperimentConfigSolve, LayerRef};
use super::parameter::{RunParameterCell, RunParameterTable};
use super::record::record_name;
use super::{
    SealedExperiment, EXPERIMENT_CONFIG_MEDIA_TYPE, EXPERIMENT_STATUS_FINISHED,
    RUN_PARAMETERS_MEDIA_TYPE,
};
use crate::artifact::local_registry::StoredDescriptor;
use crate::artifact::{ImageRef, LocalArtifact};
use anyhow::{Context, Result};
use oci_spec::image::{Descriptor, MediaType};
use std::collections::BTreeMap;

impl<'reg> SealedExperiment<'reg> {
    /// Reconstruct a sealed Experiment from a committed Experiment Artifact.
    pub fn from_artifact(artifact: LocalArtifact<'reg>) -> Result<Self> {
        let config = load_experiment_config(&artifact)?;
        let layers = artifact.layers()?;

        let records = decode_records(
            artifact.registry(),
            &layers,
            config.attachments,
            "experiment",
        )?;
        let mut runs = BTreeMap::new();
        for run in config.runs {
            let records = decode_records(
                artifact.registry(),
                &layers,
                run.attachments,
                &format!("run {}", run.run_id),
            )?;
            let solves = decode_solves(artifact.registry(), &layers, run.run_id, run.solves)?;
            if runs
                .insert(
                    run.run_id,
                    SealedRun {
                        run_id: run.run_id,
                        records,
                        solves,
                    },
                )
                .is_some()
            {
                crate::bail!("Experiment config contains duplicate Run {}", run.run_id);
            }
        }
        let run_parameters = load_run_parameters(&artifact, &layers, config.run_parameters)?;
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

    pub fn experiment_records(&self) -> &[StoredDescriptor<'reg>] {
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
    records: Vec<StoredDescriptor<'reg>>,
    solves: Vec<SealedSolve<'reg>>,
}

impl<'reg> SealedRun<'reg> {
    pub fn run_id(&self) -> u64 {
        self.run_id
    }

    pub fn records(&self) -> &[StoredDescriptor<'reg>] {
        &self.records
    }

    pub fn solves(&self) -> &[SealedSolve<'reg>] {
        &self.solves
    }
}

#[derive(Debug, Clone)]
pub struct SealedSolve<'reg> {
    solve_id: u64,
    input: StoredDescriptor<'reg>,
    output: StoredDescriptor<'reg>,
    parameters: BTreeMap<String, super::ParameterValue>,
}

impl<'reg> SealedSolve<'reg> {
    pub fn solve_id(&self) -> u64 {
        self.solve_id
    }

    pub fn input(&self) -> &StoredDescriptor<'reg> {
        &self.input
    }

    pub fn output(&self) -> &StoredDescriptor<'reg> {
        &self.output
    }

    pub fn parameters(&self) -> &BTreeMap<String, super::ParameterValue> {
        &self.parameters
    }
}

fn load_experiment_config(artifact: &LocalArtifact<'_>) -> Result<ExperimentConfig> {
    let config = artifact.get_manifest()?.config();
    if config.media_type() != &MediaType::Other(EXPERIMENT_CONFIG_MEDIA_TYPE.to_string()) {
        crate::bail!(
            "Experiment config media type is {}, expected {}",
            config.media_type(),
            EXPERIMENT_CONFIG_MEDIA_TYPE
        );
    }
    let bytes = artifact.get_blob(config.digest())?;
    let config = serde_json::from_slice::<ExperimentConfig>(&bytes)
        .context("Failed to decode Experiment config")?;
    if config.status != EXPERIMENT_STATUS_FINISHED {
        crate::bail!(
            "Experiment config status is {}, expected {}",
            config.status,
            EXPERIMENT_STATUS_FINISHED
        );
    }
    Ok(config)
}

fn decode_records<'reg>(
    registry: &'reg crate::artifact::local_registry::LocalRegistry,
    layers: &[Descriptor],
    records: Vec<LayerRef>,
    owner: &str,
) -> Result<Vec<StoredDescriptor<'reg>>> {
    let mut decoded = Vec::new();
    for layer_ref in records {
        let descriptor = resolve_layer(layers, layer_ref, &format!("{owner} record"))?.clone();
        if record_name(&descriptor).is_none() {
            crate::bail!("Record descriptor in {owner} is missing `org.ommx.record.name`");
        }
        decoded.push(
            registry
                .stored_descriptor(descriptor)
                .with_context(|| format!("Failed to decode Record in {owner}"))?,
        );
    }
    Ok(decoded)
}

fn decode_solves<'reg>(
    registry: &'reg crate::artifact::local_registry::LocalRegistry,
    layers: &[Descriptor],
    run_id: u64,
    solves: Vec<ExperimentConfigSolve>,
) -> Result<Vec<SealedSolve<'reg>>> {
    let mut decoded = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for solve in solves {
        if !seen.insert(solve.solve_id) {
            crate::bail!("Run {run_id} contains duplicate Solve {}", solve.solve_id);
        }
        let input = resolve_layer(
            layers,
            solve.input,
            &format!("run {run_id} solve {} input", solve.solve_id),
        )?
        .clone();
        validate_layer_media_type(
            &input,
            &crate::artifact::media_types::v1_instance(),
            &format!("Run {run_id} Solve {} input", solve.solve_id),
        )?;
        let output = resolve_layer(
            layers,
            solve.output,
            &format!("run {run_id} solve {} output", solve.solve_id),
        )?
        .clone();
        validate_layer_media_type(
            &output,
            &crate::artifact::media_types::v1_solution(),
            &format!("Run {run_id} Solve {} output", solve.solve_id),
        )?;
        super::parameter::ParameterSet::from_map(solve.parameters.clone())?;
        decoded.push(SealedSolve {
            solve_id: solve.solve_id,
            input: registry
                .stored_descriptor(input)
                .with_context(|| format!("Failed to decode Run {run_id} Solve input"))?,
            output: registry
                .stored_descriptor(output)
                .with_context(|| format!("Failed to decode Run {run_id} Solve output"))?,
            parameters: solve.parameters,
        });
    }
    Ok(decoded)
}

fn load_run_parameters(
    artifact: &LocalArtifact<'_>,
    layers: &[Descriptor],
    layer_ref: LayerRef,
) -> Result<RunParameterTable> {
    let descriptor = resolve_layer(layers, layer_ref, "run-parameter table")?;
    if descriptor.media_type() != &MediaType::Other(RUN_PARAMETERS_MEDIA_TYPE.to_string()) {
        crate::bail!(
            "Run-parameter descriptor media type is {}, expected {}",
            descriptor.media_type(),
            RUN_PARAMETERS_MEDIA_TYPE
        );
    }
    let bytes = artifact.get_blob(descriptor.digest())?;
    serde_json::from_slice::<RunParameterTable>(&bytes)
        .context("Failed to decode run-parameter table JSON")
}

fn resolve_layer<'a>(
    layers: &'a [Descriptor],
    layer_ref: LayerRef,
    owner: &str,
) -> Result<&'a Descriptor> {
    layers.get(layer_ref.0 as usize).ok_or_else(|| {
        anyhow::anyhow!(
            "Experiment config references {owner} layer {}, but artifact has only {} layer(s)",
            layer_ref.0,
            layers.len()
        )
    })
}

fn validate_layer_media_type(
    descriptor: &Descriptor,
    expected: &MediaType,
    owner: &str,
) -> Result<()> {
    if descriptor.media_type() != expected {
        crate::bail!(
            "{owner} media type is {}, expected {}",
            descriptor.media_type(),
            expected
        );
    }
    Ok(())
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
