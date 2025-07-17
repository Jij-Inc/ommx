mod analysis;
mod approx;
mod arbitrary;
mod clip_bounds;
mod constraint_hints;
mod convert;
mod decision_variable;
mod error;
mod evaluate;
mod log_encode;
mod new;
mod parse;
mod pass;
mod penalty;
mod reduce_binary_power;
mod serialize;
mod setter;
mod substitute;

pub use analysis::*;
pub use constraint_hints::*;
pub use error::*;
pub use log_encode::*;

use crate::{
    parse::Parse, v1, AcyclicAssignments, Constraint, ConstraintID, DecisionVariable, Evaluate,
    Function, RemovedConstraint, VariableID, VariableIDSet,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Sense {
    #[default]
    Minimize,
    Maximize,
}

/// Instance, represents a mathematical optimization problem.
///
/// Invariants
/// -----------
/// - [`Self::decision_variables`] contains all decision variables used in the problem.
/// - The keys of [`Self::constraints`] and [`Self::removed_constraints`] are disjoint sets.
/// - The keys of [`Self::decision_variable_dependency`] are not used. See also the document of [`DecisionVariableAnalysis`].
///
#[derive(Debug, Clone, PartialEq, getset::Getters, Default)]
pub struct Instance {
    #[getset(get = "pub")]
    sense: Sense,
    #[getset(get = "pub")]
    objective: Function,
    #[getset(get = "pub")]
    decision_variables: BTreeMap<VariableID, DecisionVariable>,
    #[getset(get = "pub")]
    constraints: BTreeMap<ConstraintID, Constraint>,
    #[getset(get = "pub")]
    removed_constraints: BTreeMap<ConstraintID, RemovedConstraint>,
    #[getset(get = "pub")]
    decision_variable_dependency: AcyclicAssignments,

    /// The constraint hints, i.e. some constraints are in form of one-hot, SOS1,2, or other special types.
    ///
    /// Note
    /// -----
    /// This struct does not validate the hints in mathematical sense.
    /// Only checks the decision variable and constraint IDs are valid.
    #[getset(get = "pub")]
    constraint_hints: ConstraintHints,

    // Optional fields for additional metadata.
    // These fields are public since arbitrary values can be set without validation.
    pub parameters: Option<v1::Parameters>,
    pub description: Option<v1::instance::Description>,
}

/// Optimization problem instance with parameters
///
/// Invariants
/// -----------
/// - [`Self::decision_variables`] and [`Self::parameters`] contains all decision variables and parameters used in the problem.
///   - This means every IDs appearing in the constraints and the objective function must be included in either of them.
///   - The IDs of [`Self::decision_variables`] and [`Self::parameters`] are disjoint sets.
/// - The keys of [`Self::constraints`] and [`Self::removed_constraints`] are disjoint sets.
/// - The keys of [`Self::decision_variable_dependency`] are not used. See also the document of [`DecisionVariableAnalysis`].
///
#[derive(Debug, Clone, PartialEq, getset::Getters, Default)]
pub struct ParametricInstance {
    #[getset(get = "pub")]
    sense: Sense,
    #[getset(get = "pub")]
    objective: Function,
    #[getset(get = "pub")]
    decision_variables: BTreeMap<VariableID, DecisionVariable>,
    #[getset(get = "pub")]
    parameters: BTreeMap<VariableID, v1::Parameter>,
    #[getset(get = "pub")]
    constraints: BTreeMap<ConstraintID, Constraint>,
    #[getset(get = "pub")]
    removed_constraints: BTreeMap<ConstraintID, RemovedConstraint>,
    #[getset(get = "pub")]
    decision_variable_dependency: AcyclicAssignments,

    /// The constraint hints, i.e. some constraints are in form of one-hot, SOS1,2, or other special types.
    ///
    /// Note
    /// -----
    /// This struct does not validate the hints in mathematical sense.
    /// Only checks the decision variable and constraint IDs are valid.
    #[getset(get = "pub")]
    constraint_hints: ConstraintHints,

    // Optional fields for additional metadata.
    // These fields are public since arbitrary values can be set without validation.
    pub description: Option<v1::instance::Description>,
}
