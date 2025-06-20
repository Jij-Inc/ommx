mod parse;

use crate::{ConstraintID, EvaluatedConstraint, EvaluatedDecisionVariable, VariableID};
use getset::Getters;
use std::collections::{BTreeMap, BTreeSet, HashMap};

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
    ) -> Self {
        Self {
            objective,
            evaluated_constraints,
            decision_variables,
            optimality: crate::v1::Optimality::Unspecified,
            relaxation: crate::v1::Relaxation::Unspecified,
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
    /// - A decision variable with parameters is found
    /// - The same subscript is found multiple times
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use ommx::*;
    /// # use std::collections::{HashMap, BTreeMap};
    /// # let solution = Solution::new(0.0, BTreeMap::new(), BTreeMap::new());
    /// // Assuming you have a solution with variables named "x" with subscripts [0], [1], [2]
    /// let extracted: HashMap<Vec<i64>, f64> = solution.extract_decision_variables("x")?;
    /// // extracted will contain: {vec![0] => 1.0, vec![1] => 0.0, vec![2] => 0.0}
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn extract_decision_variables(
        &self,
        name: &str,
    ) -> Result<HashMap<Vec<i64>, f64>, SolutionError> {
        let mut result = HashMap::new();

        for dv in self.decision_variables.values() {
            if let Some(dv_name) = &dv.metadata.name {
                if dv_name == name {
                    if !dv.metadata.parameters.is_empty() {
                        return Err(SolutionError::ParameterizedVariable);
                    }
                    let key = dv.metadata.subscripts.clone();
                    if result.contains_key(&key) {
                        return Err(SolutionError::DuplicateSubscript { subscripts: key });
                    }
                    result.insert(key, *dv.value());
                }
            }
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
    /// - A constraint with parameters is found
    /// - The same subscript is found multiple times
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use ommx::*;
    /// # use std::collections::{HashMap, BTreeMap};
    /// # let solution = Solution::new(0.0, BTreeMap::new(), BTreeMap::new());
    /// // Assuming you have a solution with constraints named "c" with subscripts [0], [1]
    /// let extracted: HashMap<Vec<i64>, f64> = solution.extract_constraints("c")?;
    /// // extracted will contain: {vec![0] => 0.0, vec![1] => 0.0}
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn extract_constraints(&self, name: &str) -> Result<HashMap<Vec<i64>, f64>, SolutionError> {
        let mut result = HashMap::new();

        for ec in self.evaluated_constraints.values() {
            if let Some(constraint_name) = &ec.metadata.name {
                if constraint_name == name {
                    if !ec.metadata.parameters.is_empty() {
                        return Err(SolutionError::ParameterizedConstraint);
                    }
                    let key = ec.metadata.subscripts.clone();
                    if result.contains_key(&key) {
                        return Err(SolutionError::DuplicateSubscript { subscripts: key });
                    }
                    result.insert(key, *ec.evaluated_value());
                }
            }
        }
        Ok(result)
    }
}
