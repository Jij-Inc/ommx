mod analysis;
mod approx;
mod arbitrary;
mod constraint_hints;
mod error;
mod evaluate;
mod parse;
mod pass;

use std::collections::BTreeMap;

pub use analysis::*;
pub use constraint_hints::*;
pub use error::*;

use crate::{
    parse::Parse, v1, Constraint, ConstraintID, DecisionVariable, Evaluate, Function,
    RemovedConstraint, VariableID,
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
/// - [`Self::decision_variables`] contains all decision variables used in the problem.
/// - The keys of [`Self::constraints`] and [`Self::removed_constraints`] are disjoint sets.
/// - The keys of [`Self::decision_variable_dependency`] are not used. See also the document of [`DecisionVariableAnalysis`].
///
#[derive(Debug, Clone, PartialEq, getset::Getters)]
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
    decision_variable_dependency: BTreeMap<VariableID, Function>,

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

impl Instance {
    pub fn new(
        sense: Sense,
        objective: Function,
        decision_variables: BTreeMap<VariableID, DecisionVariable>,
        constraints: BTreeMap<ConstraintID, Constraint>,
        constraint_hints: ConstraintHints,
    ) -> anyhow::Result<Self> {
        // Validate constraint_hints using Parse trait
        let hints: v1::ConstraintHints = constraint_hints.into();
        let context = (decision_variables, constraints);
        let constraint_hints = hints.parse(&context)?;

        // Validate undefined VariableID using Evaluate::required_ids
        let instance = Instance {
            sense,
            objective,
            decision_variables: context.0,
            constraints: context.1,
            removed_constraints: BTreeMap::new(),
            decision_variable_dependency: BTreeMap::new(),
            parameters: None,
            description: None,
            constraint_hints,
        };
        for id in instance.required_ids() {
            if !instance.decision_variables.contains_key(&id) {
                // FIXME: This should returns all undefined VariableIDs, not just the first one.
                return Err(InstanceError::UndefinedVariableID { id }.into());
            }
        }
        Ok(instance)
    }
}
