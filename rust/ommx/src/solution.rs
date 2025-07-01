mod parse;

use crate::{ConstraintID, EvaluatedConstraint, EvaluatedDecisionVariable, Sense, VariableID};
use getset::Getters;
use std::collections::{BTreeMap, BTreeSet};

/// Error occurred during Solution validation
#[derive(Debug, thiserror::Error)]
pub enum SolutionError {
    #[error("Inconsistent feasibility for solution: provided={provided_feasible}, computed={computed_feasible}")]
    InconsistentFeasibility {
        provided_feasible: bool,
        computed_feasible: bool,
    },

    #[error("Inconsistent feasibility (relaxed) for solution: provided={provided_feasible_relaxed}, computed={computed_feasible_relaxed}")]
    InconsistentFeasibilityRelaxed {
        provided_feasible_relaxed: bool,
        computed_feasible_relaxed: bool,
    },

    #[error("Inconsistent value for variable {id}: state={state_value}, substituted_value={substituted_value}")]
    InconsistentVariableValue {
        id: u64,
        state_value: f64,
        substituted_value: f64,
    },

    #[error("Missing value for variable {id}: not found in state and no substituted_value")]
    MissingVariableValue { id: u64 },

    #[error("Decision variable with parameters is not supported")]
    ParameterizedVariable,

    #[error("Constraint with parameters is not supported")]
    ParameterizedConstraint,

    #[error("Duplicate subscript: {subscripts:?}")]
    DuplicateSubscript { subscripts: Vec<i64> },

    #[error("Unknown constraint ID: {id:?}")]
    UnknownConstraintID { id: ConstraintID },

    #[error("No decision variables with name '{name}' found")]
    UnknownVariableName { name: String },

    #[error("No constraint with name '{name}' found")]
    UnknownConstraintName { name: String },
}

/// Single solution result with data integrity guarantees
#[derive(Debug, Clone, PartialEq, Getters)]
pub struct Solution {
    #[getset(get = "pub")]
    objective: f64,
    #[getset(get = "pub")]
    evaluated_constraints: BTreeMap<ConstraintID, EvaluatedConstraint>,
    #[getset(get = "pub")]
    decision_variables: BTreeMap<VariableID, EvaluatedDecisionVariable>,
    /// Optimality status - not guaranteed by Solution itself
    pub optimality: crate::v1::Optimality,
    /// Relaxation status - not guaranteed by Solution itself
    pub relaxation: crate::v1::Relaxation,
    #[getset(get = "pub")]
    sense: Sense,
}

impl Solution {
    /// Create a new Solution
    ///
    /// Optimality and relaxation are set to Unspecified by default.
    /// Feasibility is computed on-demand from the evaluated constraints.
    pub fn new(
        objective: f64,
        evaluated_constraints: BTreeMap<ConstraintID, EvaluatedConstraint>,
        decision_variables: BTreeMap<VariableID, EvaluatedDecisionVariable>,
        sense: Sense,
    ) -> Self {
        Self {
            objective,
            evaluated_constraints,
            decision_variables,
            optimality: crate::v1::Optimality::Unspecified,
            relaxation: crate::v1::Relaxation::Unspecified,
            sense,
        }
    }

    /// Get decision variable IDs used in this solution
    pub fn decision_variable_ids(&self) -> BTreeSet<VariableID> {
        self.decision_variables.keys().cloned().collect()
    }

    /// Get constraint IDs evaluated in this solution
    pub fn constraint_ids(&self) -> BTreeSet<ConstraintID> {
        self.evaluated_constraints.keys().cloned().collect()
    }

    /// Check if all constraints are feasible
    pub fn feasible(&self) -> bool {
        self.evaluated_constraints.values().all(|c| *c.feasible())
    }

    /// Check if all constraints are feasible in the relaxed problem
    pub fn feasible_relaxed(&self) -> bool {
        self.evaluated_constraints
            .values()
            .filter(|c| c.removed_reason().is_none())
            .all(|c| *c.feasible())
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

    /// Extract decision variables by name with subscripts as key
    ///
    /// Returns a mapping from subscripts (as a vector) to the variable's value.
    /// This is useful for extracting variables that have the same name but different subscripts.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No decision variables with the given name are found
    /// - A decision variable with parameters is found
    /// - The same subscript is found multiple times
    ///
    pub fn extract_decision_variables(
        &self,
        name: &str,
    ) -> Result<BTreeMap<Vec<i64>, f64>, SolutionError> {
        // Collect all variables with the given name
        let variables_with_name: Vec<&EvaluatedDecisionVariable> = self
            .decision_variables
            .values()
            .filter(|v| v.metadata.name.as_deref() == Some(name))
            .collect();
        if variables_with_name.is_empty() {
            return Err(SolutionError::UnknownVariableName {
                name: name.to_string(),
            });
        }

        let mut result = BTreeMap::new();
        for dv in &variables_with_name {
            if !dv.metadata.parameters.is_empty() {
                return Err(SolutionError::ParameterizedVariable);
            }
            let key = dv.metadata.subscripts.clone();
            if result.contains_key(&key) {
                return Err(SolutionError::DuplicateSubscript { subscripts: key });
            }
            result.insert(key, *dv.value());
        }
        Ok(result)
    }

    /// Extract constraints by name with subscripts as key
    ///
    /// Returns a mapping from subscripts (as a vector) to the constraint's evaluated value.
    /// This is useful for extracting constraints that have the same name but different subscripts.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No constraints with the given name are found
    /// - A constraint with parameters is found
    /// - The same subscript is found multiple times
    ///
    pub fn extract_constraints(
        &self,
        name: &str,
    ) -> Result<BTreeMap<Vec<i64>, f64>, SolutionError> {
        // Collect all constraints with the given name
        let constraints_with_name: Vec<&EvaluatedConstraint> = self
            .evaluated_constraints
            .values()
            .filter(|c| c.metadata.name.as_deref() == Some(name))
            .collect();
        if constraints_with_name.is_empty() {
            return Err(SolutionError::UnknownConstraintName {
                name: name.to_string(),
            });
        }

        let mut result = BTreeMap::new();
        for ec in &constraints_with_name {
            if !ec.metadata.parameters.is_empty() {
                return Err(SolutionError::ParameterizedConstraint);
            }
            let key = ec.metadata.subscripts.clone();
            if result.contains_key(&key) {
                return Err(SolutionError::DuplicateSubscript { subscripts: key });
            }
            result.insert(key, *ec.evaluated_value());
        }
        Ok(result)
    }

    /// Get the evaluated value of a specific constraint by ID
    pub fn get_constraint_value(&self, constraint_id: ConstraintID) -> Result<f64, SolutionError> {
        self.evaluated_constraints
            .get(&constraint_id)
            .map(|c| *c.evaluated_value())
            .ok_or(SolutionError::UnknownConstraintID { id: constraint_id })
    }

    /// Get the dual variable value of a specific constraint by ID
    pub fn get_dual_variable(
        &self,
        constraint_id: ConstraintID,
    ) -> Result<Option<f64>, SolutionError> {
        self.evaluated_constraints
            .get(&constraint_id)
            .map(|c| c.dual_variable)
            .ok_or(SolutionError::UnknownConstraintID { id: constraint_id })
    }

    /// Set the dual variable value for a specific constraint by ID
    pub fn set_dual_variable(
        &mut self,
        constraint_id: ConstraintID,
        value: Option<f64>,
    ) -> Result<(), SolutionError> {
        if let Some(constraint) = self.evaluated_constraints.get_mut(&constraint_id) {
            constraint.dual_variable = value;
            Ok(())
        } else {
            Err(SolutionError::UnknownConstraintID { id: constraint_id })
        }
    }
}
