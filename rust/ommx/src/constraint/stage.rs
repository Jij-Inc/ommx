use crate::logical_memory::LogicalMemoryProfile;
use crate::VariableIDSet;
use fnv::FnvHashMap;

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

/// Ordered participation of a constraint in the current formulation and queries.
///
/// Queries include a row when its activeness is at least the requested minimum.
/// The default includes active and checked inactive rows; selecting
/// [`Self::SkipFeasibilityCheck`] includes every retained row.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, LogicalMemoryProfile,
)]
pub enum ConstraintActiveness {
    /// Retained and evaluated, but excluded from default feasibility judgments.
    SkipFeasibilityCheck,
    /// Not imposed on the solver, but included in default feasibility judgments.
    #[default]
    Inactive,
    /// Imposed on the solver as part of the active formulation.
    Active,
}

/// Inactive membership metadata owned by a constraint collection.
///
/// This record exists only for inactive rows. The collection owns membership;
/// the skip flag distinguishes their default feasibility participation.
#[derive(Debug, Clone, PartialEq, Default, LogicalMemoryProfile)]
pub struct InactiveConstraintMetadata {
    /// Fully qualified, dot-separated path identifying the method, function,
    /// or application that deactivated the constraint.
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
    /// Exclude this row from default feasibility queries, while still evaluating it.
    pub skip_feasibility_check: bool,
}

impl InactiveConstraintMetadata {
    /// The activeness represented by this inactive membership record.
    pub fn activeness(&self) -> ConstraintActiveness {
        if self.skip_feasibility_check {
            ConstraintActiveness::SkipFeasibilityCheck
        } else {
            ConstraintActiveness::Inactive
        }
    }
}

impl From<InactiveConstraintMetadata> for crate::v2::InactiveConstraintMetadata {
    fn from(reason: InactiveConstraintMetadata) -> Self {
        Self {
            reason: reason.reason,
            parameters: reason.parameters.into_iter().collect(),
            skip_feasibility_check: reason.skip_feasibility_check,
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
///
/// Stores numeric results, with no retained feasibility decision or tolerance.
/// Feasibility queries receive their own absolute tolerance.
#[derive(Debug, Clone, PartialEq)]
pub struct EvaluatedData {
    pub evaluated_value: f64,
    pub used_decision_variable_ids: VariableIDSet,
    pub dual_variable: Option<f64>,
}

/// Data carried by a constraint in the Sampled stage.
///
/// Sample IDs in dual values, when present, must match evaluated values.
/// Feasibility queries receive their own absolute tolerance; the numeric results
/// do not retain a feasibility decision or its tolerance.
#[derive(Debug, Clone, PartialEq)]
pub struct SampledData {
    pub evaluated_values: crate::Sampled<f64>,
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
