mod analysis;
mod constraint_hints;
mod parse;

pub use analysis::*;
pub use constraint_hints::*;

use crate::{
    v1, Constraint, ConstraintID, DecisionVariable, Function, RemovedConstraint, VariableID,
};
use fnv::FnvHashMap;

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
    decision_variables: FnvHashMap<VariableID, DecisionVariable>,
    constraints: FnvHashMap<ConstraintID, Constraint>,
    removed_constraints: FnvHashMap<ConstraintID, RemovedConstraint>,
    decision_variable_dependency: FnvHashMap<VariableID, Function>,
    parameters: Option<v1::Parameters>,
    description: Option<v1::instance::Description>,
    constraint_hints: ConstraintHints,
}
