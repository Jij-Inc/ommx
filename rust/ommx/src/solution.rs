mod parse;
mod serialize;

use crate::{
    ConstraintID, EvaluatedConstraint, EvaluatedDecisionVariable, EvaluatedNamedFunction,
    NamedFunctionID, Sense, VariableID,
};
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

    #[deprecated(
        note = "Parameters are now ignored in extract_decision_variables and extract_all_decision_variables"
    )]
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

    #[error("Unknown named function ID: {id:?}")]
    UnknownNamedFunctionID { id: NamedFunctionID },

    #[error("No named function with name '{name}' found")]
    UnknownNamedFunctionName { name: String },

    #[error("Named function with parameters is not supported")]
    ParameterizedNamedFunction,

    #[error("Required field is missing: {field}")]
    MissingRequiredField { field: &'static str },

    #[error("Decision variable key {key:?} does not match value's id {value_id:?}")]
    InconsistentDecisionVariableID {
        key: VariableID,
        value_id: VariableID,
    },

    #[error("Constraint key {key:?} does not match value's id {value_id:?}")]
    InconsistentConstraintID {
        key: ConstraintID,
        value_id: ConstraintID,
    },

    #[error(
        "Variable ID {id:?} used in constraint {constraint_id:?} is not in decision_variables"
    )]
    UndefinedVariableInConstraint {
        id: VariableID,
        constraint_id: ConstraintID,
    },
}

/// Single solution result with data integrity guarantees
///
/// Invariants
/// -----------
/// - The keys of [`Self::decision_variables`] match the `id()` of their values.
/// - The keys of [`Self::evaluated_constraints`] match the `id()` of their values.
/// - [`Self::decision_variables`] contains all variable IDs referenced in `used_decision_variable_ids` of each constraint.
///
/// Note
/// -----
/// - [`Self::optimality`] is determined by the solver, not validated by this struct.
/// - [`Self::relaxation`] is a record of operations, not validated by this struct.
#[derive(Debug, Clone, PartialEq, Getters)]
pub struct Solution {
    #[getset(get = "pub")]
    objective: f64,
    #[getset(get = "pub")]
    evaluated_constraints: BTreeMap<ConstraintID, EvaluatedConstraint>,
    #[getset(get = "pub")]
    evaluated_named_functions: BTreeMap<NamedFunctionID, EvaluatedNamedFunction>,
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
    /// Create a new Solution without validation.
    ///
    /// # Deprecated
    /// This constructor does not validate invariants.
    /// Use [`SolutionBuilder::build`] for validated construction,
    /// or [`SolutionBuilder::build_unchecked`] if invariants are guaranteed by construction.
    #[deprecated(
        since = "2.5.0",
        note = "Use Solution::builder().build() for validated construction, or Solution::builder().build_unchecked() for unchecked construction"
    )]
    pub fn new(
        objective: f64,
        evaluated_constraints: BTreeMap<ConstraintID, EvaluatedConstraint>,
        decision_variables: BTreeMap<VariableID, EvaluatedDecisionVariable>,
        sense: Sense,
    ) -> Self {
        // SAFETY: This is a deprecated method that doesn't validate invariants.
        // Callers are responsible for ensuring data integrity.
        unsafe {
            Solution::builder()
                .objective(objective)
                .evaluated_constraints(evaluated_constraints)
                .decision_variables(decision_variables)
                .sense(sense)
                .build_unchecked()
                .expect("All required fields are provided")
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

    /// Get named function IDs evaluated in this solution
    pub fn named_function_ids(&self) -> BTreeSet<NamedFunctionID> {
        self.evaluated_named_functions.keys().cloned().collect()
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
    /// Note: Parameters in decision variable metadata are ignored. Only subscripts are used as keys.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No decision variables with the given name are found
    /// - The same subscript is found multiple times (which can happen when parameters differ)
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
    /// Note: Parameters in decision variable metadata are ignored. Only subscripts are used as keys.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The same name and subscript combination is found multiple times (which can happen when parameters differ)
    ///
    pub fn extract_all_decision_variables(
        &self,
    ) -> Result<BTreeMap<String, BTreeMap<Vec<i64>, f64>>, SolutionError> {
        let mut result: BTreeMap<String, BTreeMap<Vec<i64>, f64>> = BTreeMap::new();

        for dv in self.decision_variables.values() {
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

    /// Get all unique named function names in this solution
    ///
    /// Returns a set of all unique function names that have at least one named function.
    /// Named functions without names are not included.
    pub fn named_function_names(&self) -> BTreeSet<String> {
        self.evaluated_named_functions
            .values()
            .filter_map(|nf| nf.name().clone())
            .collect()
    }

    /// Extract named functions by name with subscripts as key
    ///
    /// Returns a mapping from subscripts (as a vector) to the function's evaluated value.
    /// This is useful for extracting named functions that have the same name but different subscripts.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No named functions with the given name are found
    /// - A named function with parameters is found
    /// - The same subscript is found multiple times
    pub fn extract_named_functions(
        &self,
        name: &str,
    ) -> Result<BTreeMap<Vec<i64>, f64>, SolutionError> {
        // Collect all named functions with the given name
        let functions_with_name: Vec<&EvaluatedNamedFunction> = self
            .evaluated_named_functions
            .values()
            .filter(|nf| nf.name().as_deref() == Some(name))
            .collect();
        if functions_with_name.is_empty() {
            return Err(SolutionError::UnknownNamedFunctionName {
                name: name.to_string(),
            });
        }

        let mut result = BTreeMap::new();
        for nf in &functions_with_name {
            if !nf.parameters().is_empty() {
                return Err(SolutionError::ParameterizedNamedFunction);
            }
            let key = nf.subscripts().clone();
            if result.contains_key(&key) {
                return Err(SolutionError::DuplicateSubscript { subscripts: key });
            }
            result.insert(key, nf.evaluated_value());
        }
        Ok(result)
    }

    /// Extract all named functions grouped by name
    ///
    /// Returns a mapping from function name to a mapping from subscripts to evaluated values.
    /// This is useful for extracting all named functions at once in a structured format.
    /// Named functions without names are not included in the result.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - A named function with parameters is found
    /// - The same name and subscript combination is found multiple times
    pub fn extract_all_named_functions(
        &self,
    ) -> Result<BTreeMap<String, BTreeMap<Vec<i64>, f64>>, SolutionError> {
        let mut result: BTreeMap<String, BTreeMap<Vec<i64>, f64>> = BTreeMap::new();

        for nf in self.evaluated_named_functions.values() {
            let name = match nf.name() {
                Some(n) => n.clone(),
                None => continue, // Skip named functions without names
            };

            if !nf.parameters().is_empty() {
                return Err(SolutionError::ParameterizedNamedFunction);
            }

            let subscripts = nf.subscripts().clone();
            let value = nf.evaluated_value();

            let funcs_map = result.entry(name).or_default();
            if funcs_map.contains_key(&subscripts) {
                return Err(SolutionError::DuplicateSubscript { subscripts });
            }
            funcs_map.insert(subscripts, value);
        }

        Ok(result)
    }

    /// Creates a new [`SolutionBuilder`].
    pub fn builder() -> SolutionBuilder {
        SolutionBuilder::new()
    }
}

/// Builder for creating [`Solution`] with validation.
///
/// # Example
/// ```
/// use ommx::{Solution, Sense};
/// use std::collections::BTreeMap;
///
/// let solution = Solution::builder()
///     .objective(0.0)
///     .evaluated_constraints(BTreeMap::new())
///     .decision_variables(BTreeMap::new())
///     .sense(Sense::Minimize)
///     .build()
///     .unwrap();
/// ```
#[derive(Debug, Clone, Default)]
pub struct SolutionBuilder {
    objective: Option<f64>,
    evaluated_constraints: Option<BTreeMap<ConstraintID, EvaluatedConstraint>>,
    evaluated_named_functions: BTreeMap<NamedFunctionID, EvaluatedNamedFunction>,
    decision_variables: Option<BTreeMap<VariableID, EvaluatedDecisionVariable>>,
    sense: Option<Sense>,
    optimality: crate::v1::Optimality,
    relaxation: crate::v1::Relaxation,
}

impl SolutionBuilder {
    /// Creates a new `SolutionBuilder` with all fields unset.
    pub fn new() -> Self {
        Self {
            optimality: crate::v1::Optimality::Unspecified,
            relaxation: crate::v1::Relaxation::Unspecified,
            ..Default::default()
        }
    }

    /// Sets the objective value.
    pub fn objective(mut self, objective: f64) -> Self {
        self.objective = Some(objective);
        self
    }

    /// Sets the evaluated constraints.
    pub fn evaluated_constraints(
        mut self,
        evaluated_constraints: BTreeMap<ConstraintID, EvaluatedConstraint>,
    ) -> Self {
        self.evaluated_constraints = Some(evaluated_constraints);
        self
    }

    /// Sets the evaluated named functions.
    pub fn evaluated_named_functions(
        mut self,
        evaluated_named_functions: BTreeMap<NamedFunctionID, EvaluatedNamedFunction>,
    ) -> Self {
        self.evaluated_named_functions = evaluated_named_functions;
        self
    }

    /// Sets the decision variables.
    pub fn decision_variables(
        mut self,
        decision_variables: BTreeMap<VariableID, EvaluatedDecisionVariable>,
    ) -> Self {
        self.decision_variables = Some(decision_variables);
        self
    }

    /// Sets the optimization sense.
    pub fn sense(mut self, sense: Sense) -> Self {
        self.sense = Some(sense);
        self
    }

    /// Sets the optimality status.
    pub fn optimality(mut self, optimality: crate::v1::Optimality) -> Self {
        self.optimality = optimality;
        self
    }

    /// Sets the relaxation status.
    pub fn relaxation(mut self, relaxation: crate::v1::Relaxation) -> Self {
        self.relaxation = relaxation;
        self
    }

    /// Builds the `Solution` with full validation.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Required fields (`objective`, `evaluated_constraints`, `decision_variables`, `sense`) are not set
    /// - Decision variable keys don't match their value's `id()`
    /// - Constraint keys don't match their value's `id()`
    /// - Variables referenced in constraints' `used_decision_variable_ids` are not in `decision_variables`
    pub fn build(self) -> anyhow::Result<Solution> {
        let objective = self
            .objective
            .ok_or(SolutionError::MissingRequiredField { field: "objective" })?;
        let evaluated_constraints =
            self.evaluated_constraints
                .ok_or(SolutionError::MissingRequiredField {
                    field: "evaluated_constraints",
                })?;
        let decision_variables =
            self.decision_variables
                .ok_or(SolutionError::MissingRequiredField {
                    field: "decision_variables",
                })?;
        let sense = self
            .sense
            .ok_or(SolutionError::MissingRequiredField { field: "sense" })?;

        // Validate decision variable keys match their id
        for (key, value) in &decision_variables {
            if key != value.id() {
                return Err(SolutionError::InconsistentDecisionVariableID {
                    key: *key,
                    value_id: *value.id(),
                }
                .into());
            }
        }

        // Validate constraint keys match their id
        for (key, value) in &evaluated_constraints {
            if key != value.id() {
                return Err(SolutionError::InconsistentConstraintID {
                    key: *key,
                    value_id: *value.id(),
                }
                .into());
            }
        }

        // Validate all used_decision_variable_ids are in decision_variables
        for constraint in evaluated_constraints.values() {
            for var_id in constraint.used_decision_variable_ids() {
                if !decision_variables.contains_key(var_id) {
                    return Err(SolutionError::UndefinedVariableInConstraint {
                        id: *var_id,
                        constraint_id: *constraint.id(),
                    }
                    .into());
                }
            }
        }

        Ok(Solution {
            objective,
            evaluated_constraints,
            evaluated_named_functions: self.evaluated_named_functions,
            decision_variables,
            optimality: self.optimality,
            relaxation: self.relaxation,
            sense: Some(sense),
        })
    }

    /// Builds the `Solution` without invariant validation.
    ///
    /// Builds the `Solution` without invariant validation.
    ///
    /// # Safety
    /// This method does not validate that the Solution invariants hold.
    /// The caller must ensure:
    /// - Decision variable keys match their value's `id()`
    /// - Constraint keys match their value's `id()`
    /// - All `used_decision_variable_ids` in constraints exist in `decision_variables`
    ///
    /// Use [`Self::build`] for validated construction.
    /// This method is useful when invariants are guaranteed by construction,
    /// such as when creating a Solution from `Instance::evaluate`.
    ///
    /// # Errors
    /// Returns an error only if required fields are not set.
    pub unsafe fn build_unchecked(self) -> anyhow::Result<Solution> {
        let objective = self
            .objective
            .ok_or(SolutionError::MissingRequiredField { field: "objective" })?;
        let evaluated_constraints =
            self.evaluated_constraints
                .ok_or(SolutionError::MissingRequiredField {
                    field: "evaluated_constraints",
                })?;
        let decision_variables =
            self.decision_variables
                .ok_or(SolutionError::MissingRequiredField {
                    field: "decision_variables",
                })?;
        let sense = self
            .sense
            .ok_or(SolutionError::MissingRequiredField { field: "sense" })?;

        Ok(Solution {
            objective,
            evaluated_constraints,
            evaluated_named_functions: self.evaluated_named_functions,
            decision_variables,
            optimality: self.optimality,
            relaxation: self.relaxation,
            sense: Some(sense),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Coefficient, Constraint, Evaluate, Function};

    #[test]
    fn test_total_violation_l1_all_satisfied() {
        // All constraints satisfied → total violation = 0
        let mut constraints = BTreeMap::new();

        // Equality constraint: f(x) = 0.0001 (near zero, but not exactly zero due to Coefficient restrictions)
        let c1 = Constraint::equal_to_zero(
            ConstraintID::from(1),
            Function::Constant(Coefficient::try_from(0.0001).unwrap()),
        );
        let state = crate::v1::State::default();
        constraints.insert(
            ConstraintID::from(1),
            c1.evaluate(&state, crate::ATol::default()).unwrap(),
        );

        // Inequality constraint: f(x) = -1.0 ≤ 0 (satisfied)
        let c2 = Constraint::less_than_or_equal_to_zero(
            ConstraintID::from(2),
            Function::Constant(Coefficient::try_from(-1.0).unwrap()),
        );
        constraints.insert(
            ConstraintID::from(2),
            c2.evaluate(&state, crate::ATol::default()).unwrap(),
        );

        // SAFETY: Test data is constructed to satisfy invariants
        let solution = unsafe {
            Solution::builder()
                .objective(0.0)
                .evaluated_constraints(constraints)
                .decision_variables(BTreeMap::new())
                .sense(Sense::Minimize)
                .build_unchecked()
                .unwrap()
        };

        // L1: |0.0001| + max(0, -1.0) = 0.0001 + 0 = 0.0001
        assert_eq!(solution.total_violation_l1(), 0.0001);
    }

    #[test]
    fn test_total_violation_l1_mixed() {
        // Mix of satisfied and violated constraints
        let mut constraints = BTreeMap::new();
        let state = crate::v1::State::default();

        // Equality constraint violated: f(x) = 2.5
        let c1 = Constraint::equal_to_zero(
            ConstraintID::from(1),
            Function::Constant(Coefficient::try_from(2.5).unwrap()),
        );
        constraints.insert(
            ConstraintID::from(1),
            c1.evaluate(&state, crate::ATol::default()).unwrap(),
        );

        // Inequality constraint violated: f(x) = 1.5 > 0
        let c2 = Constraint::less_than_or_equal_to_zero(
            ConstraintID::from(2),
            Function::Constant(Coefficient::try_from(1.5).unwrap()),
        );
        constraints.insert(
            ConstraintID::from(2),
            c2.evaluate(&state, crate::ATol::default()).unwrap(),
        );

        // Inequality constraint satisfied: f(x) = -0.5 ≤ 0
        let c3 = Constraint::less_than_or_equal_to_zero(
            ConstraintID::from(3),
            Function::Constant(Coefficient::try_from(-0.5).unwrap()),
        );
        constraints.insert(
            ConstraintID::from(3),
            c3.evaluate(&state, crate::ATol::default()).unwrap(),
        );

        // SAFETY: Test data is constructed to satisfy invariants
        let solution = unsafe {
            Solution::builder()
                .objective(0.0)
                .evaluated_constraints(constraints)
                .decision_variables(BTreeMap::new())
                .sense(Sense::Minimize)
                .build_unchecked()
                .unwrap()
        };

        // L1: |2.5| + max(0, 1.5) + max(0, -0.5) = 2.5 + 1.5 + 0 = 4.0
        assert_eq!(solution.total_violation_l1(), 4.0);
    }

    #[test]
    fn test_total_violation_l2_mixed() {
        // Same constraints as L1 test
        let mut constraints = BTreeMap::new();
        let state = crate::v1::State::default();

        // Equality constraint violated: f(x) = 2.5
        let c1 = Constraint::equal_to_zero(
            ConstraintID::from(1),
            Function::Constant(Coefficient::try_from(2.5).unwrap()),
        );
        constraints.insert(
            ConstraintID::from(1),
            c1.evaluate(&state, crate::ATol::default()).unwrap(),
        );

        // Inequality constraint violated: f(x) = 1.5 > 0
        let c2 = Constraint::less_than_or_equal_to_zero(
            ConstraintID::from(2),
            Function::Constant(Coefficient::try_from(1.5).unwrap()),
        );
        constraints.insert(
            ConstraintID::from(2),
            c2.evaluate(&state, crate::ATol::default()).unwrap(),
        );

        // Inequality constraint satisfied: f(x) = -0.5 ≤ 0
        let c3 = Constraint::less_than_or_equal_to_zero(
            ConstraintID::from(3),
            Function::Constant(Coefficient::try_from(-0.5).unwrap()),
        );
        constraints.insert(
            ConstraintID::from(3),
            c3.evaluate(&state, crate::ATol::default()).unwrap(),
        );

        // SAFETY: Test data is constructed to satisfy invariants
        let solution = unsafe {
            Solution::builder()
                .objective(0.0)
                .evaluated_constraints(constraints)
                .decision_variables(BTreeMap::new())
                .sense(Sense::Minimize)
                .build_unchecked()
                .unwrap()
        };

        // L2: (2.5)² + (1.5)² + 0² = 6.25 + 2.25 + 0 = 8.5
        assert_eq!(solution.total_violation_l2(), 8.5);
    }

    #[test]
    fn test_total_violation_empty() {
        // No constraints → total violation = 0
        // SAFETY: Test data is constructed to satisfy invariants
        let solution = unsafe {
            Solution::builder()
                .objective(0.0)
                .evaluated_constraints(BTreeMap::new())
                .decision_variables(BTreeMap::new())
                .sense(Sense::Minimize)
                .build_unchecked()
                .unwrap()
        };

        assert_eq!(solution.total_violation_l1(), 0.0);
        assert_eq!(solution.total_violation_l2(), 0.0);
    }

    #[test]
    fn test_total_violation_equality_negative() {
        // Test with negative value for equality constraint
        let mut constraints = BTreeMap::new();
        let state = crate::v1::State::default();

        // Equality constraint: f(x) = -3.0
        let c1 = Constraint::equal_to_zero(
            ConstraintID::from(1),
            Function::Constant(Coefficient::try_from(-3.0).unwrap()),
        );
        constraints.insert(
            ConstraintID::from(1),
            c1.evaluate(&state, crate::ATol::default()).unwrap(),
        );

        // SAFETY: Test data is constructed to satisfy invariants
        let solution = unsafe {
            Solution::builder()
                .objective(0.0)
                .evaluated_constraints(constraints)
                .decision_variables(BTreeMap::new())
                .sense(Sense::Minimize)
                .build_unchecked()
                .unwrap()
        };

        // L1: |-3.0| = 3.0
        assert_eq!(solution.total_violation_l1(), 3.0);
        // L2: (-3.0)² = 9.0
        assert_eq!(solution.total_violation_l2(), 9.0);
    }

    #[test]
    fn test_extract_parameterized_variable_success() {
        use crate::{
            decision_variable::{DecisionVariable, DecisionVariableMetadata, Kind},
            EvaluatedDecisionVariable, Sense, VariableID,
        };

        // Create a parameterized decision variable (should succeed - parameters are ignored)
        let mut decision_variables = BTreeMap::new();

        let mut dv = DecisionVariable::new(
            VariableID::from(1),
            Kind::Continuous,
            crate::Bound::new(f64::NEG_INFINITY, f64::INFINITY).unwrap(),
            None,
            crate::ATol::default(),
        )
        .unwrap();
        dv.metadata = DecisionVariableMetadata {
            name: Some("x".to_string()),
            subscripts: vec![0],
            parameters: {
                let mut params = fnv::FnvHashMap::default();
                params.insert("param1".to_string(), "value1".to_string());
                params
            },
            ..Default::default()
        };

        decision_variables.insert(
            VariableID::from(1),
            EvaluatedDecisionVariable::new(dv, 1.0, crate::ATol::default()).unwrap(),
        );

        // SAFETY: Test data is constructed to satisfy invariants
        let solution = unsafe {
            Solution::builder()
                .objective(0.0)
                .evaluated_constraints(BTreeMap::new())
                .decision_variables(decision_variables)
                .sense(Sense::Minimize)
                .build_unchecked()
                .unwrap()
        };

        // Test that extracting parameterized variable succeeds (parameters are ignored)
        let result = solution.extract_decision_variables("x");
        assert!(result.is_ok());
        let vars = result.unwrap();
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[&vec![0]], 1.0);
    }

    #[test]
    fn test_extract_duplicate_subscripts_error() {
        use crate::{
            decision_variable::{DecisionVariable, DecisionVariableMetadata, Kind},
            EvaluatedDecisionVariable, Sense, VariableID,
        };

        // Create two variables with same name and subscripts but different parameters
        let mut decision_variables = BTreeMap::new();

        // First variable
        let mut dv1 = DecisionVariable::new(
            VariableID::from(1),
            Kind::Continuous,
            crate::Bound::new(f64::NEG_INFINITY, f64::INFINITY).unwrap(),
            None,
            crate::ATol::default(),
        )
        .unwrap();
        dv1.metadata = DecisionVariableMetadata {
            name: Some("x".to_string()),
            subscripts: vec![0],
            parameters: {
                let mut params = fnv::FnvHashMap::default();
                params.insert("param".to_string(), "value1".to_string());
                params
            },
            ..Default::default()
        };

        decision_variables.insert(
            VariableID::from(1),
            EvaluatedDecisionVariable::new(dv1, 1.0, crate::ATol::default()).unwrap(),
        );

        // Second variable with same name and subscripts
        let mut dv2 = DecisionVariable::new(
            VariableID::from(2),
            Kind::Continuous,
            crate::Bound::new(f64::NEG_INFINITY, f64::INFINITY).unwrap(),
            None,
            crate::ATol::default(),
        )
        .unwrap();
        dv2.metadata = DecisionVariableMetadata {
            name: Some("x".to_string()),
            subscripts: vec![0], // Same subscripts
            parameters: {
                let mut params = fnv::FnvHashMap::default();
                params.insert("param".to_string(), "value2".to_string());
                params
            },
            ..Default::default()
        };

        decision_variables.insert(
            VariableID::from(2),
            EvaluatedDecisionVariable::new(dv2, 2.0, crate::ATol::default()).unwrap(),
        );

        // SAFETY: Test data is constructed to satisfy invariants
        let solution = unsafe {
            Solution::builder()
                .objective(0.0)
                .evaluated_constraints(BTreeMap::new())
                .decision_variables(decision_variables)
                .sense(Sense::Minimize)
                .build_unchecked()
                .unwrap()
        };

        // Test that extracting variables with duplicate subscripts fails
        let result = solution.extract_decision_variables("x");
        assert!(matches!(
            result,
            Err(SolutionError::DuplicateSubscript { .. })
        ));
    }

    #[test]
    fn test_builder_missing_required_field() {
        // Missing objective
        let err = Solution::builder()
            .evaluated_constraints(BTreeMap::new())
            .decision_variables(BTreeMap::new())
            .sense(Sense::Minimize)
            .build()
            .unwrap_err();
        let solution_err = err.downcast_ref::<SolutionError>().unwrap();
        assert!(matches!(
            solution_err,
            SolutionError::MissingRequiredField { field: "objective" }
        ));

        // Missing sense
        let err = Solution::builder()
            .objective(0.0)
            .evaluated_constraints(BTreeMap::new())
            .decision_variables(BTreeMap::new())
            .build()
            .unwrap_err();
        let solution_err = err.downcast_ref::<SolutionError>().unwrap();
        assert!(matches!(
            solution_err,
            SolutionError::MissingRequiredField { field: "sense" }
        ));
    }

    #[test]
    fn test_builder_inconsistent_decision_variable_id() {
        use crate::DecisionVariable;

        let var_id_1 = VariableID::from(1);
        let var_id_2 = VariableID::from(2);
        let dv = DecisionVariable::binary(var_id_1);
        let evaluated_dv = EvaluatedDecisionVariable::new(dv, 1.0, crate::ATol::default()).unwrap();

        // Map key (2) doesn't match value's id (1)
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(var_id_2, evaluated_dv);

        let err = Solution::builder()
            .objective(0.0)
            .evaluated_constraints(BTreeMap::new())
            .decision_variables(decision_variables)
            .sense(Sense::Minimize)
            .build()
            .unwrap_err();
        let solution_err = err.downcast_ref::<SolutionError>().unwrap();
        assert!(matches!(
            solution_err,
            SolutionError::InconsistentDecisionVariableID { key, value_id }
                if *key == var_id_2 && *value_id == var_id_1
        ));
    }

    #[test]
    fn test_builder_inconsistent_constraint_id() {
        let state = crate::v1::State::default();
        let constraint_id_1 = ConstraintID::from(1);
        let constraint_id_2 = ConstraintID::from(2);

        let c = Constraint::equal_to_zero(
            constraint_id_1,
            Function::Constant(Coefficient::try_from(1.0).unwrap()),
        );
        let evaluated_c = c.evaluate(&state, crate::ATol::default()).unwrap();

        // Map key (2) doesn't match value's id (1)
        let mut evaluated_constraints = BTreeMap::new();
        evaluated_constraints.insert(constraint_id_2, evaluated_c);

        let err = Solution::builder()
            .objective(0.0)
            .evaluated_constraints(evaluated_constraints)
            .decision_variables(BTreeMap::new())
            .sense(Sense::Minimize)
            .build()
            .unwrap_err();
        let solution_err = err.downcast_ref::<SolutionError>().unwrap();
        assert!(matches!(
            solution_err,
            SolutionError::InconsistentConstraintID { key, value_id }
                if *key == constraint_id_2 && *value_id == constraint_id_1
        ));
    }

    #[test]
    fn test_builder_undefined_variable_in_constraint() {
        use crate::linear;

        let state = crate::v1::State::from(std::collections::HashMap::from([(1, 1.0)]));
        let constraint_id = ConstraintID::from(1);
        let var_id = VariableID::from(1);

        // Constraint uses variable ID 1
        let c = Constraint::equal_to_zero(constraint_id, Function::from(linear!(1)));
        let evaluated_c = c.evaluate(&state, crate::ATol::default()).unwrap();

        let mut evaluated_constraints = BTreeMap::new();
        evaluated_constraints.insert(constraint_id, evaluated_c);

        // decision_variables is empty, so variable ID 1 is undefined
        let err = Solution::builder()
            .objective(0.0)
            .evaluated_constraints(evaluated_constraints)
            .decision_variables(BTreeMap::new())
            .sense(Sense::Minimize)
            .build()
            .unwrap_err();
        let solution_err = err.downcast_ref::<SolutionError>().unwrap();
        assert!(matches!(
            solution_err,
            SolutionError::UndefinedVariableInConstraint { id, constraint_id: cid }
                if *id == var_id && *cid == constraint_id
        ));
    }

    #[test]
    fn test_builder_success() {
        use crate::DecisionVariable;

        let var_id = VariableID::from(1);
        let dv = DecisionVariable::binary(var_id);
        let evaluated_dv = EvaluatedDecisionVariable::new(dv, 1.0, crate::ATol::default()).unwrap();

        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(var_id, evaluated_dv);

        let solution = Solution::builder()
            .objective(42.0)
            .evaluated_constraints(BTreeMap::new())
            .decision_variables(decision_variables)
            .sense(Sense::Maximize)
            .build()
            .unwrap();

        assert_eq!(*solution.objective(), 42.0);
        assert_eq!(*solution.sense(), Some(Sense::Maximize));
    }
}
