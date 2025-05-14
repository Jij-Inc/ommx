mod analysis;
mod approx;
mod arbitrary;
mod constraint_hints;
mod parse;

use std::collections::BTreeMap;

pub use analysis::*;
pub use constraint_hints::*;

use crate::{
    v1, Constraint, ConstraintID, DecisionVariable, Function, RemovedConstraint, VariableID,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Sense {
    Minimize,
    Maximize,
}

/// Instance, represents a mathematical optimization problem.
///
/// Invariants
/// -----------
/// - All `VariableID`s in `Function`s contained both directly and indirectly must be keys of `decision_variables`.
/// - Key of `constraints` and `removed_constraints` are disjoint.
/// - The keys of `decision_variable_dependency` are also keys of `decision_variables`.
///
#[derive(Debug, Clone, PartialEq)]
pub struct Instance {
    sense: Sense,
    objective: Function,
    decision_variables: BTreeMap<VariableID, DecisionVariable>,
    constraints: BTreeMap<ConstraintID, Constraint>,
    removed_constraints: BTreeMap<ConstraintID, RemovedConstraint>,
    decision_variable_dependency: BTreeMap<VariableID, Function>,
    parameters: Option<v1::Parameters>,
    description: Option<v1::instance::Description>,
    constraint_hints: ConstraintHints,
}
