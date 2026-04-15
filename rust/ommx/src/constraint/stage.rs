use crate::{SampleID, VariableIDSet};
use fnv::FnvHashMap;
use std::collections::BTreeMap;

use super::Constraint;

/// Trait that defines the stage-specific data a constraint carries at each lifecycle phase.
///
/// The type parameter `C` is the constraint struct itself (e.g. `Constraint<S>`),
/// allowing different constraint types to have different stage data for the same stage.
pub trait Stage<C> {
    type Data;
}

/// The constraint as defined in the problem, before evaluation.
#[derive(Debug, Clone, PartialEq)]
pub struct Created;

/// The constraint has been removed (relaxed) from the active set.
#[derive(Debug, Clone, PartialEq)]
pub struct Removed;

// ===== Common types =====

/// Reason why a constraint was removed/relaxed.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RemovedReason {
    /// Short reason (e.g. method or application name that removed the constraint).
    pub reason: String,
    /// Arbitrary key-value parameters for debugging.
    pub parameters: FnvHashMap<String, String>,
}

// ===== Stage data types for regular Constraint =====

/// Data carried by a regular constraint in the Created stage.
#[derive(Debug, Clone, PartialEq)]
pub struct CreatedData {
    pub function: crate::Function,
}

/// Data carried by a regular constraint in the Removed stage.
#[derive(Debug, Clone, PartialEq)]
pub struct RemovedData {
    pub function: crate::Function,
    pub removed_reason: RemovedReason,
}

/// The constraint has been evaluated against a single state.
#[derive(Debug, Clone, PartialEq)]
pub struct Evaluated;

/// The constraint has been evaluated against multiple samples.
#[derive(Debug, Clone)]
pub struct Sampled;

// ===== Stage data types for Evaluated/Sampled =====

/// Data carried by a constraint in the Evaluated stage.
#[derive(Debug, Clone, PartialEq)]
pub struct EvaluatedData {
    pub evaluated_value: f64,
    pub feasible: bool,
    pub used_decision_variable_ids: VariableIDSet,
    pub dual_variable: Option<f64>,
    pub removed_reason: Option<RemovedReason>,
}

/// Data carried by a constraint in the Sampled stage.
#[derive(Debug, Clone)]
pub struct SampledData {
    pub evaluated_values: crate::Sampled<f64>,
    pub feasible: BTreeMap<SampleID, bool>,
    pub used_decision_variable_ids: VariableIDSet,
    pub dual_variables: Option<crate::Sampled<f64>>,
    pub removed_reason: Option<RemovedReason>,
}

// ===== Stage implementations for Constraint =====

impl Stage<Constraint<Created>> for Created {
    type Data = CreatedData;
}

impl Stage<Constraint<Removed>> for Removed {
    type Data = RemovedData;
}

impl Stage<Constraint<Evaluated>> for Evaluated {
    type Data = EvaluatedData;
}

impl Stage<Constraint<Sampled>> for Sampled {
    type Data = SampledData;
}
