mod parse;
mod serialize;

use crate::{ConstraintID, EvaluatedConstraint, EvaluatedDecisionVariable, Sense, VariableID};
use getset::Getters;
use std::collections::{BTreeMap, BTreeSet};

/// Error occurred during Solution validation
#[non_exhaustive]
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
    sense: Option<Sense>,
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
            sense: Some(sense),
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

    /// Check if all decision variables satisfy their kind and bound constraints
    pub fn feasible_decision_variables(&self) -> bool {
        self.decision_variables
            .values()
            .all(|dv| dv.is_valid(crate::ATol::default()))
    }

    /// Check if all constraints are feasible
    ///
    /// Note: This only checks constraints, not decision variable bounds/kinds.
    /// - To check both constraints and decision variables, use [`feasible()`](Self::feasible)
    /// - To check only decision variables, use [`feasible_decision_variables()`](Self::feasible_decision_variables)
    pub fn feasible_constraints(&self) -> bool {
        self.evaluated_constraints.values().all(|c| *c.feasible())
    }

    /// Check if all constraints and decision variables are feasible
    ///
    /// This is the most comprehensive feasibility check, verifying:
    /// - All constraints are satisfied (via [`feasible_constraints()`](Self::feasible_constraints))
    /// - All decision variables satisfy their bounds and kinds (via [`feasible_decision_variables()`](Self::feasible_decision_variables))
    pub fn feasible(&self) -> bool {
        self.feasible_constraints() && self.feasible_decision_variables()
    }

    /// Check if all constraints are feasible in the relaxed problem
    ///
    /// Note: This only checks constraints, not decision variable bounds/kinds.
    /// - To check both constraints and decision variables, use [`feasible_relaxed()`](Self::feasible_relaxed)
    /// - To check only decision variables, use [`feasible_decision_variables()`](Self::feasible_decision_variables)
    pub fn feasible_constraints_relaxed(&self) -> bool {
        self.evaluated_constraints
            .values()
            .filter(|c| c.removed_reason().is_none())
            .all(|c| *c.feasible())
    }

    /// Check if all constraints and decision variables are feasible in the relaxed problem
    ///
    /// This checks:
    /// - Relaxed constraints are satisfied (via [`feasible_constraints_relaxed()`](Self::feasible_constraints_relaxed))
    /// - All decision variables satisfy their bounds and kinds (via [`feasible_decision_variables()`](Self::feasible_decision_variables))
    pub fn feasible_relaxed(&self) -> bool {
        self.feasible_constraints_relaxed() && self.feasible_decision_variables()
    }

    /// Calculate total constraint violation using L1 norm (sum of absolute violations)
    ///
    /// Returns the sum of violations across all constraints (including removed constraints):
    /// - For equality constraints: `Σ|f(x)|`
    /// - For inequality constraints: `Σmax(0, f(x))`
    ///
    /// This metric is useful for:
    /// - Assessing solution quality when constraints are violated
    /// - Penalty method implementations
    /// - Comparing different solutions
    pub fn total_violation_l1(&self) -> f64 {
        self.evaluated_constraints
            .values()
            .map(|c| c.violation())
            .sum()
    }

    /// Calculate total constraint violation using L2 norm squared (sum of squared violations)
    ///
    /// Returns the sum of squared violations across all constraints (including removed constraints):
    /// - For equality constraints: `Σ(f(x))²`
    /// - For inequality constraints: `Σ(max(0, f(x)))²`
    ///
    /// This metric is useful for:
    /// - Penalty methods that use quadratic penalties
    /// - Emphasizing larger violations over smaller ones
    /// - Smooth optimization objectives
    pub fn total_violation_l2(&self) -> f64 {
        self.evaluated_constraints
            .values()
            .map(|c| {
                let v = c.violation();
                v * v
            })
            .sum()
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

    /// Get all unique decision variable names in this solution
    ///
    /// Returns a set of all unique variable names that have at least one named variable.
    /// Variables without names are not included.
    pub fn decision_variable_names(&self) -> BTreeSet<String> {
        self.decision_variables
            .values()
            .filter_map(|dv| dv.metadata.name.clone())
            .collect()
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

    /// Extract all decision variables grouped by name
    ///
    /// Returns a mapping from variable name to a mapping from subscripts to values.
    /// This is useful for extracting all variables at once in a structured format.
    /// Variables without names are not included in the result.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - A decision variable with parameters is found
    /// - The same name and subscript combination is found multiple times
    ///
    pub fn extract_all_decision_variables(
        &self,
    ) -> Result<BTreeMap<String, BTreeMap<Vec<i64>, f64>>, SolutionError> {
        let mut result: BTreeMap<String, BTreeMap<Vec<i64>, f64>> = BTreeMap::new();

        for dv in self.decision_variables.values() {
            if !dv.metadata.parameters.is_empty() {
                return Err(SolutionError::ParameterizedVariable);
            }

            let name = match &dv.metadata.name {
                Some(n) => n.clone(),
                None => continue, // Skip variables without names
            };

            let subscripts = dv.metadata.subscripts.clone();
            let value = *dv.value();

            let vars_map = result.entry(name).or_default();
            if vars_map.contains_key(&subscripts) {
                return Err(SolutionError::DuplicateSubscript { subscripts });
            }
            vars_map.insert(subscripts, value);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Coefficient, Constraint, Equality, Evaluate, Function};

    #[test]
    fn test_total_violation_l1_all_satisfied() {
        // All constraints satisfied → total violation = 0
        let mut constraints = BTreeMap::new();

        // Equality constraint: f(x) = 0.0001 (near zero, but not exactly zero due to Coefficient restrictions)
        let c1 = Constraint {
            id: ConstraintID::from(1),
            equality: Equality::EqualToZero,
            function: Function::Constant(Coefficient::try_from(0.0001).unwrap()),
            name: None,
            subscripts: vec![],
            parameters: Default::default(),
            description: None,
        };
        let state = crate::v1::State::default();
        constraints.insert(
            ConstraintID::from(1),
            c1.evaluate(&state, crate::ATol::default()).unwrap(),
        );

        // Inequality constraint: f(x) = -1.0 ≤ 0 (satisfied)
        let c2 = Constraint {
            id: ConstraintID::from(2),
            equality: Equality::LessThanOrEqualToZero,
            function: Function::Constant(Coefficient::try_from(-1.0).unwrap()),
            name: None,
            subscripts: vec![],
            parameters: Default::default(),
            description: None,
        };
        constraints.insert(
            ConstraintID::from(2),
            c2.evaluate(&state, crate::ATol::default()).unwrap(),
        );

        let solution = Solution::new(0.0, constraints, BTreeMap::new(), Sense::Minimize);

        // L1: |0.0001| + max(0, -1.0) = 0.0001 + 0 = 0.0001
        assert_eq!(solution.total_violation_l1(), 0.0001);
    }

    #[test]
    fn test_total_violation_l1_mixed() {
        // Mix of satisfied and violated constraints
        let mut constraints = BTreeMap::new();
        let state = crate::v1::State::default();

        // Equality constraint violated: f(x) = 2.5
        let c1 = Constraint {
            id: ConstraintID::from(1),
            equality: Equality::EqualToZero,
            function: Function::Constant(Coefficient::try_from(2.5).unwrap()),
            name: None,
            subscripts: vec![],
            parameters: Default::default(),
            description: None,
        };
        constraints.insert(
            ConstraintID::from(1),
            c1.evaluate(&state, crate::ATol::default()).unwrap(),
        );

        // Inequality constraint violated: f(x) = 1.5 > 0
        let c2 = Constraint {
            id: ConstraintID::from(2),
            equality: Equality::LessThanOrEqualToZero,
            function: Function::Constant(Coefficient::try_from(1.5).unwrap()),
            name: None,
            subscripts: vec![],
            parameters: Default::default(),
            description: None,
        };
        constraints.insert(
            ConstraintID::from(2),
            c2.evaluate(&state, crate::ATol::default()).unwrap(),
        );

        // Inequality constraint satisfied: f(x) = -0.5 ≤ 0
        let c3 = Constraint {
            id: ConstraintID::from(3),
            equality: Equality::LessThanOrEqualToZero,
            function: Function::Constant(Coefficient::try_from(-0.5).unwrap()),
            name: None,
            subscripts: vec![],
            parameters: Default::default(),
            description: None,
        };
        constraints.insert(
            ConstraintID::from(3),
            c3.evaluate(&state, crate::ATol::default()).unwrap(),
        );

        let solution = Solution::new(0.0, constraints, BTreeMap::new(), Sense::Minimize);

        // L1: |2.5| + max(0, 1.5) + max(0, -0.5) = 2.5 + 1.5 + 0 = 4.0
        assert_eq!(solution.total_violation_l1(), 4.0);
    }

    #[test]
    fn test_total_violation_l2_mixed() {
        // Same constraints as L1 test
        let mut constraints = BTreeMap::new();
        let state = crate::v1::State::default();

        // Equality constraint violated: f(x) = 2.5
        let c1 = Constraint {
            id: ConstraintID::from(1),
            equality: Equality::EqualToZero,
            function: Function::Constant(Coefficient::try_from(2.5).unwrap()),
            name: None,
            subscripts: vec![],
            parameters: Default::default(),
            description: None,
        };
        constraints.insert(
            ConstraintID::from(1),
            c1.evaluate(&state, crate::ATol::default()).unwrap(),
        );

        // Inequality constraint violated: f(x) = 1.5 > 0
        let c2 = Constraint {
            id: ConstraintID::from(2),
            equality: Equality::LessThanOrEqualToZero,
            function: Function::Constant(Coefficient::try_from(1.5).unwrap()),
            name: None,
            subscripts: vec![],
            parameters: Default::default(),
            description: None,
        };
        constraints.insert(
            ConstraintID::from(2),
            c2.evaluate(&state, crate::ATol::default()).unwrap(),
        );

        // Inequality constraint satisfied: f(x) = -0.5 ≤ 0
        let c3 = Constraint {
            id: ConstraintID::from(3),
            equality: Equality::LessThanOrEqualToZero,
            function: Function::Constant(Coefficient::try_from(-0.5).unwrap()),
            name: None,
            subscripts: vec![],
            parameters: Default::default(),
            description: None,
        };
        constraints.insert(
            ConstraintID::from(3),
            c3.evaluate(&state, crate::ATol::default()).unwrap(),
        );

        let solution = Solution::new(0.0, constraints, BTreeMap::new(), Sense::Minimize);

        // L2: (2.5)² + (1.5)² + 0² = 6.25 + 2.25 + 0 = 8.5
        assert_eq!(solution.total_violation_l2(), 8.5);
    }

    #[test]
    fn test_total_violation_empty() {
        // No constraints → total violation = 0
        let solution = Solution::new(0.0, BTreeMap::new(), BTreeMap::new(), Sense::Minimize);

        assert_eq!(solution.total_violation_l1(), 0.0);
        assert_eq!(solution.total_violation_l2(), 0.0);
    }

    #[test]
    fn test_total_violation_equality_negative() {
        // Test with negative value for equality constraint
        let mut constraints = BTreeMap::new();
        let state = crate::v1::State::default();

        // Equality constraint: f(x) = -3.0
        let c1 = Constraint {
            id: ConstraintID::from(1),
            equality: Equality::EqualToZero,
            function: Function::Constant(Coefficient::try_from(-3.0).unwrap()),
            name: None,
            subscripts: vec![],
            parameters: Default::default(),
            description: None,
        };
        constraints.insert(
            ConstraintID::from(1),
            c1.evaluate(&state, crate::ATol::default()).unwrap(),
        );

        let solution = Solution::new(0.0, constraints, BTreeMap::new(), Sense::Minimize);

        // L1: |-3.0| = 3.0
        assert_eq!(solution.total_violation_l1(), 3.0);
        // L2: (-3.0)² = 9.0
        assert_eq!(solution.total_violation_l2(), 9.0);
    }
}
