//! Serialized Experiment structure stored in the OCI config blob.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LayerRef(pub u32);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExperimentConfig {
    pub status: String,
    pub attachments: Vec<LayerRef>,
    pub runs: Vec<ExperimentConfigRun>,
    pub run_parameters: LayerRef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExperimentConfigRun {
    pub run_id: u64,
    #[serde(default = "default_run_status")]
    pub status: String,
    pub attachments: Vec<LayerRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace: Option<LayerRef>,
    #[serde(default)]
    pub solves: Vec<ExperimentConfigSolve>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExperimentConfigSolve {
    pub solve_id: u64,
    pub input: LayerRef,
    pub output: LayerRef,
    pub adapter: String,
    #[serde(default = "default_adapter_options")]
    pub adapter_options: String,
}

fn default_adapter_options() -> String {
    "{}".to_string()
}

fn default_run_status() -> String {
    "finished".to_string()
}
