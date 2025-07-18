use super::*;
use crate::{linear, v1, Function, VariableID};
use anyhow::Result;
use num::Zero;

impl Instance {
    #[cfg_attr(doc, katexit::katexit)]
    /// Convert constraints to penalty terms in the objective function.
    ///
    /// This method transforms a constrained optimization problem into an unconstrained one by
    /// adding penalty terms to the objective function. Each constraint $f(x) = 0$ or $f(x) \leq 0$
    /// is converted to a penalty term $\lambda \cdot f(x)^2$ where $\lambda$ is a penalty parameter.
    ///
    /// # Returns
    ///
    /// A `ParametricInstance` where:
    /// - The objective function includes penalty terms for each constraint
    /// - Each constraint has a corresponding penalty parameter
    /// - All original constraints are moved to `removed_constraints`
    /// - The constraint list is empty
    ///
    /// # Example
    ///
    /// For a problem:
    ///
    /// $$
    /// \begin{align*}
    ///   \min & \quad x + y \\
    ///   \text{s.t.} & \quad x + y \leq 1 \\
    ///   & \quad x - y = 0
    /// \end{align*}
    /// $$
    ///
    /// The penalty method transforms it to:
    ///
    /// $$
    /// \min \quad x + y + \lambda_1 \cdot (x + y - 1)^2 + \lambda_2 \cdot (x - y)^2
    /// $$
    ///
    /// where $\lambda_1$ and $\lambda_2$ are penalty parameters.
    pub fn penalty_method(self) -> Result<ParametricInstance> {
        let mut max_id = 0;

        // Find the maximum ID among decision variables
        for id in self.decision_variables.keys() {
            max_id = max_id.max(id.into_inner());
        }

        // Find the maximum ID among parameters (if any)
        if let Some(params) = &self.parameters {
            for id in params.entries.keys() {
                max_id = max_id.max(*id);
            }
        }

        let id_base = max_id + 1;
        let mut objective = self.objective.clone();
        let mut parameters = BTreeMap::new();
        let mut removed_constraints = BTreeMap::new();

        for (i, (constraint_id, constraint)) in self.constraints.into_iter().enumerate() {
            let parameter_id = VariableID::from(id_base + i as u64);
            let parameter = v1::Parameter {
                id: parameter_id.into_inner(),
                name: Some("penalty_weight".to_string()),
                subscripts: vec![constraint_id.into_inner() as i64],
                ..Default::default()
            };

            let f = constraint.function.clone();
            // Add penalty term: λ * f(x)^2
            let penalty_term = Function::from(linear!(parameter_id)) * f.clone() * f;
            objective += penalty_term;

            // Create removed constraint
            let removed_constraint = RemovedConstraint {
                constraint: constraint.clone(),
                removed_reason: "penalty_method".to_string(),
                removed_reason_parameters: {
                    let mut map = std::collections::HashMap::default();
                    map.insert(
                        "parameter_id".to_string(),
                        parameter_id.into_inner().to_string(),
                    );
                    map
                },
            };

            parameters.insert(parameter_id, parameter);
            removed_constraints.insert(constraint_id, removed_constraint);
        }

        Ok(ParametricInstance {
            sense: self.sense,
            objective,
            decision_variables: self.decision_variables,
            parameters,
            constraints: BTreeMap::new(),
            removed_constraints,
            decision_variable_dependency: self.decision_variable_dependency,
            constraint_hints: self.constraint_hints,
            description: self.description,
        })
    }

    #[cfg_attr(doc, katexit::katexit)]
    /// Convert constraints to penalty terms using a single penalty parameter.
    ///
    /// This method is similar to `penalty_method` but uses a single penalty parameter $\lambda$
    /// for all constraints. The penalty term is the sum of squared constraint violations:
    /// $\lambda \cdot \sum_i f_i(x)^2$ where $f_i(x)$ represents each constraint.
    ///
    /// # Returns
    ///
    /// A `ParametricInstance` where:
    /// - The objective function includes a single penalty term for all constraints
    /// - One penalty parameter controls the penalty for all constraints
    /// - All original constraints are moved to `removed_constraints`
    /// - The constraint list is empty
    ///
    /// # Example
    ///
    /// For a problem:
    ///
    /// $$
    /// \begin{align*}
    ///   \min & \quad x + y \\
    ///   \text{s.t.} & \quad x + y \leq 1 \\
    ///   & \quad x - y = 0
    /// \end{align*}
    /// $$
    ///
    /// The uniform penalty method transforms it to:
    ///
    /// $$
    /// \min \quad x + y + \lambda \cdot [(x + y - 1)^2 + (x - y)^2]
    /// $$
    ///
    /// where $\lambda$ is the single penalty parameter.
    pub fn uniform_penalty_method(self) -> Result<ParametricInstance> {
        // Early return if no constraints
        if self.constraints.is_empty() {
            return Ok(ParametricInstance {
                sense: self.sense,
                objective: self.objective,
                decision_variables: self.decision_variables,
                parameters: BTreeMap::new(),
                constraints: BTreeMap::new(),
                removed_constraints: BTreeMap::new(),
                decision_variable_dependency: self.decision_variable_dependency,
                constraint_hints: self.constraint_hints,
                description: self.description,
            });
        }

        let mut max_id = 0;

        // Find the maximum ID among decision variables
        for id in self.decision_variables.keys() {
            max_id = max_id.max(id.into_inner());
        }

        // Find the maximum ID among parameters (if any)
        if let Some(params) = &self.parameters {
            for id in params.entries.keys() {
                max_id = max_id.max(*id);
            }
        }

        let parameter_id = VariableID::from(max_id + 1);
        let mut objective = self.objective.clone();
        let parameter = v1::Parameter {
            id: parameter_id.into_inner(),
            name: Some("uniform_penalty_weight".to_string()),
            ..Default::default()
        };

        let mut removed_constraints = BTreeMap::new();
        let mut quad_sum = Function::zero();

        for (constraint_id, constraint) in self.constraints.into_iter() {
            let f = constraint.function.clone();
            quad_sum += f.clone() * f;

            // Create removed constraint
            let removed_constraint = RemovedConstraint {
                constraint: constraint.clone(),
                removed_reason: "uniform_penalty_method".to_string(),
                removed_reason_parameters: Default::default(),
            };

            removed_constraints.insert(constraint_id, removed_constraint);
        }

        objective += Function::from(linear!(parameter_id)) * quad_sum;

        let mut parameters = BTreeMap::new();
        parameters.insert(parameter_id, parameter);

        Ok(ParametricInstance {
            sense: self.sense,
            objective,
            decision_variables: self.decision_variables,
            parameters,
            constraints: BTreeMap::new(),
            removed_constraints,
            decision_variable_dependency: self.decision_variable_dependency,
            constraint_hints: self.constraint_hints,
            description: self.description,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, constraint::Equality, linear, DecisionVariable, Sense};
    use std::collections::BTreeMap;

    /// Helper function to create a test instance with two decision variables and two constraints
    fn create_test_instance_with_constraints() -> Instance {
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(
            VariableID::from(1),
            DecisionVariable::continuous(VariableID::from(1)),
        );
        decision_variables.insert(
            VariableID::from(2),
            DecisionVariable::continuous(VariableID::from(2)),
        );

        let objective = Function::from(linear!(1) + linear!(2));

        let mut constraints = BTreeMap::new();
        constraints.insert(
            ConstraintID::from(1),
            Constraint {
                id: ConstraintID::from(1),
                function: Function::from(linear!(1) + linear!(2) + coeff!(-1.0)),
                equality: Equality::LessThanOrEqualToZero,
                name: None,
                subscripts: Vec::new(),
                parameters: Default::default(),
                description: None,
            },
        );
        constraints.insert(
            ConstraintID::from(2),
            Constraint {
                id: ConstraintID::from(2),
                function: Function::from(linear!(1) + coeff!(-1.0) * linear!(2)),
                equality: Equality::EqualToZero,
                name: None,
                subscripts: Vec::new(),
                parameters: Default::default(),
                description: None,
            },
        );

        Instance::new(Sense::Minimize, objective, decision_variables, constraints).unwrap()
    }

    /// Helper function to verify penalty method properties
    fn verify_penalty_method_properties(
        original_objective: Function,
        original_constraint_count: usize,
        parametric_instance: &ParametricInstance,
        expected_param_count: usize,
        expected_param_name: &str,
    ) {
        // Check that constraints are removed
        assert_eq!(parametric_instance.constraints.len(), 0);
        assert_eq!(
            parametric_instance.removed_constraints.len(),
            original_constraint_count
        );

        // Check that correct number of parameters are created
        assert_eq!(parametric_instance.parameters.len(), expected_param_count);

        // Check parameter names
        for parameter in parametric_instance.parameters.values() {
            assert_eq!(parameter.name, Some(expected_param_name.to_string()));
        }

        // Verify ID disjointness
        let dv_ids: std::collections::BTreeSet<_> = parametric_instance
            .decision_variables
            .keys()
            .cloned()
            .collect();
        let p_ids: std::collections::BTreeSet<_> =
            parametric_instance.parameters.keys().cloned().collect();
        assert!(dv_ids.is_disjoint(&p_ids));

        // Verify zero penalty weight behavior
        use crate::v1::Parameters;
        use ::approx::AbsDiffEq;
        
        let parameters = Parameters {
            entries: p_ids.iter().map(|id| (id.into_inner(), 0.0)).collect(),
        };
        let substituted = parametric_instance
            .clone()
            .with_parameters(parameters)
            .unwrap();
        
        assert!(substituted
            .objective
            .abs_diff_eq(&original_objective, crate::ATol::default()));
        assert_eq!(substituted.constraints.len(), 0);
    }

    #[test]
    fn test_penalty_method() {
        let instance = create_test_instance_with_constraints();
        let original_objective = instance.objective.clone();
        let original_constraint_count = instance.constraints.len();
        let parametric_instance = instance.penalty_method().unwrap();
        
        verify_penalty_method_properties(
            original_objective,
            original_constraint_count,
            &parametric_instance,
            2, // Two parameters expected (one per constraint)
            "penalty_weight",
        );
    }

    #[test]
    fn test_uniform_penalty_method() {
        let instance = create_test_instance_with_constraints();
        let original_objective = instance.objective.clone();
        let original_constraint_count = instance.constraints.len();
        let parametric_instance = instance.uniform_penalty_method().unwrap();
        
        verify_penalty_method_properties(
            original_objective,
            original_constraint_count,
            &parametric_instance,
            1, // One parameter expected (shared for all constraints)
            "uniform_penalty_weight",
        );
    }

    #[test]
    fn test_penalty_methods_with_no_constraints() {
        // Create instance without constraints
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(
            VariableID::from(1),
            DecisionVariable::continuous(VariableID::from(1)),
        );

        let objective = Function::from(linear!(1));
        let constraints = BTreeMap::new();

        let instance = Instance::new(
            Sense::Minimize,
            objective.clone(),
            decision_variables,
            constraints,
        )
        .unwrap();

        // Test penalty_method
        let parametric_instance = instance.clone().penalty_method().unwrap();
        assert_eq!(parametric_instance.parameters.len(), 0);
        assert_eq!(parametric_instance.constraints.len(), 0);
        assert_eq!(parametric_instance.removed_constraints.len(), 0);
        assert_eq!(parametric_instance.objective, objective);

        // Test uniform_penalty_method
        let parametric_instance = instance.uniform_penalty_method().unwrap();
        assert_eq!(parametric_instance.parameters.len(), 0);
        assert_eq!(parametric_instance.constraints.len(), 0);
        assert_eq!(parametric_instance.removed_constraints.len(), 0);
        assert_eq!(parametric_instance.objective, objective);
    }
}
