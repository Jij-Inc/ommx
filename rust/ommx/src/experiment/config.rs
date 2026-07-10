//! Serialized Experiment structure stored in the OCI config blob.

use super::attachment::AttachmentTable;
use serde::{Deserialize, Serialize};

pub(crate) const CURRENT_EXPERIMENT_CONFIG_FORMAT_VERSION: u32 = 2;
const LEGACY_EXPERIMENT_CONFIG_FORMAT_VERSION: u32 = 1;

fn legacy_experiment_config_format_version() -> u32 {
    LEGACY_EXPERIMENT_CONFIG_FORMAT_VERSION
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LayerRef(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExperimentConfig {
    /// Version of the Experiment config JSON schema.
    ///
    /// Version 1 Solve outputs are limited to `Solution` layers. Version 2
    /// also permits `SampleSet` layers.
    #[serde(default = "legacy_experiment_config_format_version")]
    pub format_version: u32,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_image_name: Option<String>,
    pub attachments: AttachmentTable<LayerRef>,
    pub runs: Vec<ExperimentConfigRun>,
    pub run_parameters: LayerRef,
}

impl ExperimentConfig {
    pub(crate) fn validate_format_version(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            (LEGACY_EXPERIMENT_CONFIG_FORMAT_VERSION
                ..=CURRENT_EXPERIMENT_CONFIG_FORMAT_VERSION)
                .contains(&self.format_version),
            "Unsupported Experiment config format version: data has format_version={}, but this SDK supports versions {} through {}",
            self.format_version,
            LEGACY_EXPERIMENT_CONFIG_FORMAT_VERSION,
            CURRENT_EXPERIMENT_CONFIG_FORMAT_VERSION,
        );
        Ok(())
    }

    pub(crate) fn supports_sample_set_solve_output(&self) -> bool {
        self.format_version >= 2
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExperimentConfigRun {
    pub run_id: u64,
    #[serde(default = "default_run_status")]
    pub status: String,
    pub attachments: AttachmentTable<LayerRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace: Option<LayerRef>,
    #[serde(default)]
    pub solves: Vec<ExperimentConfigSolve>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExperimentConfigSolve {
    pub solve_id: u64,
    #[serde(default = "default_solve_status")]
    pub status: String,
    pub input: LayerRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<LayerRef>,
    pub adapter: String,
    #[serde(default = "default_adapter_options")]
    pub adapter_options: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<LayerRef>,
}

fn default_adapter_options() -> String {
    "{}".to_string()
}

fn default_run_status() -> String {
    "finished".to_string()
}

fn default_solve_status() -> String {
    "finished".to_string()
}
