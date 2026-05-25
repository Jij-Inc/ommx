//! Serialized Experiment structure stored in the OCI config blob.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
    pub attachments: Vec<LayerRef>,
    #[serde(default)]
    pub solves: Vec<ExperimentConfigSolve>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExperimentConfigSolve {
    pub solve_id: u64,
    pub input: LayerRef,
    pub output: LayerRef,
    #[serde(default)]
    pub parameters: BTreeMap<String, String>,
}
