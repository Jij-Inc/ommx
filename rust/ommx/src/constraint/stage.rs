use crate::logical_memory::LogicalMemoryProfile;
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

/// The created stage, before evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Created;

// ===== Common types =====

/// Reason why a constraint was removed/relaxed.
#[derive(Debug, Clone, PartialEq, Default, LogicalMemoryProfile)]
pub struct RemovedReason {
    /// Fully qualified, dot-separated path identifying the method, function,
    /// or application that removed the constraint.
    ///
    /// External libraries are expected to mutate constraint collections (e.g.
    /// solver adapters, preprocessing passes), so the convention is to write a
    /// stable identifier that can be traced back to the originating code.
    /// Segments follow the host language's namespacing (e.g. Rust module path,
    /// Python dotted path).
    ///
    /// Examples:
    /// - `"ommx.Instance.convert_one_hot_to_constraint"`
    /// - `"ommx.Instance.penalty_method"`
    /// - `"my_adapter.Preprocessor.drop_trivial"`
    pub reason: String,
    /// Arbitrary key-value parameters for debugging.
    pub parameters: FnvHashMap<String, String>,
}

impl From<RemovedReason> for crate::v2::RemovedReason {
    fn from(reason: RemovedReason) -> Self {
        Self {
            reason: reason.reason,
            parameters: reason.parameters.into_iter().collect(),
        }
    }
}

// ===== Stage data types for regular Constraint =====

/// Data carried by a regular constraint in the Created stage.
#[derive(Debug, Clone, PartialEq, LogicalMemoryProfile)]
pub struct CreatedData {
    pub function: crate::Function,
}

/// The evaluated stage, after evaluation against a single state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Evaluated;

/// The sampled stage, after evaluation against multiple samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Sampled;

// ===== Stage data types for Evaluated/Sampled =====

/// Data carried by a constraint in the Evaluated stage.
#[derive(Debug, Clone, PartialEq)]
pub struct EvaluatedData {
    pub evaluated_value: f64,
    pub feasible: bool,
    pub used_decision_variable_ids: VariableIDSet,
    pub dual_variable: Option<f64>,
}

/// Data carried by a constraint in the Sampled stage.
#[derive(Debug, Clone)]
pub struct SampledData {
    pub evaluated_values: crate::Sampled<f64>,
    pub feasible: BTreeMap<SampleID, bool>,
    pub used_decision_variable_ids: VariableIDSet,
    pub dual_variables: Option<crate::Sampled<f64>>,
}

// ===== Stage implementations for Constraint =====

impl Stage<Constraint<Created>> for Created {
    type Data = CreatedData;
}

impl Stage<Constraint<Evaluated>> for Evaluated {
    type Data = EvaluatedData;
}

impl Stage<Constraint<Sampled>> for Sampled {
    type Data = SampledData;
}
