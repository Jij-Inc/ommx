//! Serialized Experiment structure stored in the OCI config blob.

use super::attachment::AttachmentTable;
use super::{ExperimentLifecycle, RunLifecycle};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LayerRef(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExperimentConfig {
    #[serde(flatten)]
    pub lifecycle: ExperimentLifecycle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_image_name: Option<String>,
    pub attachments: AttachmentTable<LayerRef>,
    pub runs: Vec<ExperimentConfigRun>,
    pub run_parameters: LayerRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExperimentConfigRun {
    pub run_id: u64,
    #[serde(flatten)]
    pub lifecycle: RunLifecycle,
    pub attachments: AttachmentTable<LayerRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace: Option<LayerRef>,
    #[serde(default)]
    pub solves: Vec<ExperimentConfigSolve>,
    #[serde(default)]
    pub samplings: Vec<ExperimentConfigSampling>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExperimentConfigSampling {
    pub sampling_id: u64,
    #[serde(default = "default_sampling_status")]
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

fn default_solve_status() -> String {
    "finished".to_string()
}

fn default_sampling_status() -> String {
    "finished".to_string()
}
