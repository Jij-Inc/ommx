mod parse;

use crate::{ConstraintID, EvaluatedConstraint, EvaluatedDecisionVariable, VariableID};
use getset::Getters;
use std::collections::BTreeMap;

/// Single solution result with data integrity guarantees
#[derive(Debug, Clone, PartialEq, Getters)]
pub struct Solution {
    #[getset(get = "pub")]
    objective: f64,
    #[getset(get = "pub")]
    evaluated_constraints: BTreeMap<ConstraintID, EvaluatedConstraint>,
    #[getset(get = "pub")]
    decision_variables: BTreeMap<VariableID, EvaluatedDecisionVariable>,
    #[getset(get = "pub")]
    feasible: bool,
    #[getset(get = "pub")]
    feasible_relaxed: bool,
    #[getset(get = "pub")]
    optimality: crate::v1::Optimality,
    #[getset(get = "pub")]
    relaxation: crate::v1::Relaxation,
}

impl Solution {
    /// Create a new Solution
    pub fn new(
        objective: f64,
        evaluated_constraints: BTreeMap<ConstraintID, EvaluatedConstraint>,
        decision_variables: BTreeMap<VariableID, EvaluatedDecisionVariable>,
        feasible: bool,
        feasible_relaxed: bool,
        optimality: crate::v1::Optimality,
        relaxation: crate::v1::Relaxation,
    ) -> Self {
        Self {
            objective,
            evaluated_constraints,
            decision_variables,
            feasible,
            feasible_relaxed,
            optimality,
            relaxation,
        }
    }

    /// Get decision variable IDs used in this solution
    pub fn decision_variable_ids(&self) -> std::collections::BTreeSet<u64> {
        self.decision_variables
            .keys()
            .map(|id| id.into_inner())
            .collect()
    }

    /// Get constraint IDs evaluated in this solution
    pub fn constraint_ids(&self) -> std::collections::BTreeSet<crate::ConstraintID> {
        self.evaluated_constraints.keys().cloned().collect()
    }

    /// Check if all constraints are feasible
    pub fn is_feasible(&self) -> bool {
        *self.feasible()
    }

    /// Check if all constraints are feasible in the relaxed problem
    pub fn is_feasible_relaxed(&self) -> bool {
        *self.feasible_relaxed()
    }

    /// Generate state from decision variables (for backward compatibility)
    pub fn state(&self) -> crate::v1::State {
        let entries = self
            .decision_variables
            .iter()
            .map(|(id, dv)| (id.into_inner(), *dv.value()))
            .collect();
        crate::v1::State { entries }
    }
}
