use super::*;
use crate::{linear, Function, ParameterLabel, VariableID};
use anyhow::Result;
use std::collections::{BTreeMap, BTreeSet};

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
        let (instance, _) = self.penalty_method_with_parameter_ids()?;
        Ok(instance)
    }

    fn penalty_method_with_parameter_ids(
        self,
    ) -> Result<(ParametricInstance, BTreeMap<ConstraintID, VariableID>)> {
        self.ensure_penalty_method_supported("penalty_method")?;

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
        let mut parameters = ParameterTable::default();
        let mut parameter_ids = BTreeMap::new();
        let mut constraint_collection = self.constraint_collection;
        let mut removals = BTreeMap::new();
        for (parameter_offset, (&constraint_id, constraint)) in
            constraint_collection.active().iter().enumerate()
        {
            let parameter_offset = u64::try_from(parameter_offset)?;
            let parameter_id = VariableID::from(id_base + parameter_offset);
            let parameter_label = ParameterLabel {
                name: Some("penalty_weight".to_string()),
                subscripts: vec![constraint_id.into_inner() as i64],
                ..Default::default()
            };

            let f = constraint.function().clone();
            // Add penalty term: λ * f(x)^2
            let mut penalty_term = Function::from(linear!(parameter_id));
            penalty_term.try_mul_assign_in_place(&f)?;
            penalty_term.try_mul_assign_in_place(&f)?;
            objective.try_add_assign_in_place(penalty_term)?;

            let removed_reason = crate::constraint::RemovedReason {
                reason: "ommx.Instance.penalty_method".to_string(),
                parameters: {
                    let mut map = fnv::FnvHashMap::default();
                    map.insert(
                        "parameter_id".to_string(),
                        parameter_id.into_inner().to_string(),
                    );
                    map
                },
            };

            parameters.insert(parameter_id, parameter_label)?;
            parameter_ids.insert(constraint_id, parameter_id);
            removals.insert(constraint_id, (constraint.clone(), removed_reason));
        }
        constraint_collection.move_active_rows_to_removed(removals)?;

        Ok((
            ParametricInstance {
                sense: self.sense,
                objective,
                decision_variables: self.decision_variables,
                parameters,
                constraint_collection,
                indicator_constraint_collection: self.indicator_constraint_collection,
                one_hot_constraint_collection: self.one_hot_constraint_collection,
                sos1_constraint_collection: self.sos1_constraint_collection,
                decision_variable_dependency: self.decision_variable_dependency,
                description: self.description,
                named_functions: self.named_functions,
                annotations: self.annotations,
            },
            parameter_ids,
        ))
    }

    #[cfg_attr(doc, katexit::katexit)]
    /// Convert every active regular constraint to a penalty term with its fixed weight.
    ///
    /// The keys of `weights` must be exactly the active regular constraint IDs.
    /// Constraint `id` with body $f_{id}(x)$ contributes
    /// $\lambda_{id} f_{id}(x)^2$ using `weights[&id]`, with the same
    /// mathematical and provenance semantics as [`Self::penalty_method`].
    /// Generated penalty parameters are materialized immediately, so this
    /// operation mutates the [`Instance`] in place rather than returning a
    /// [`ParametricInstance`].
    ///
    /// When no active regular constraints exist, an empty `weights` map is an
    /// identity operation. Otherwise, conversion and parameter materialization
    /// are performed on an internal clone and committed only after both
    /// succeed.
    ///
    /// # Errors
    ///
    /// Returns an error if `weights` does not cover exactly the active regular
    /// constraints, or if [`Self::penalty_method`] or
    /// [`ParametricInstance::with_parameters`] fails. Any error leaves the
    /// instance unchanged.
    pub fn penalty_method_with_fixed_weights(
        &mut self,
        weights: &BTreeMap<ConstraintID, f64>,
    ) -> crate::Result<()> {
        self.ensure_penalty_method_supported("penalty_method")?;
        self.ensure_fixed_penalty_weight_ids(weights)?;
        if self.constraints().is_empty() {
            return Ok(());
        }

        let (parametric, parameter_ids) = self.clone().penalty_method_with_parameter_ids()?;
        let parameter_values = parameter_ids
            .into_iter()
            .map(|(constraint_id, parameter_id)| (parameter_id, weights[&constraint_id]))
            .collect();
        self.commit_materialized_penalty(parametric, parameter_values)
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
        let (instance, _) = self.uniform_penalty_method_with_parameter_id()?;
        Ok(instance)
    }

    fn uniform_penalty_method_with_parameter_id(
        self,
    ) -> Result<(ParametricInstance, Option<VariableID>)> {
        self.ensure_penalty_method_supported("uniform_penalty_method")?;

        // Early return if no active constraints (preserve any existing removed constraints)
        if self.constraints().is_empty() {
            return Ok((
                ParametricInstance {
                    sense: self.sense,
                    objective: self.objective,
                    decision_variables: self.decision_variables,
                    parameters: ParameterTable::default(),
                    constraint_collection: self.constraint_collection,
                    indicator_constraint_collection: self.indicator_constraint_collection,
                    one_hot_constraint_collection: self.one_hot_constraint_collection,
                    sos1_constraint_collection: self.sos1_constraint_collection,
                    decision_variable_dependency: self.decision_variable_dependency,
                    description: self.description,
                    named_functions: self.named_functions,
                    annotations: self.annotations,
                },
                None,
            ));
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
        let parameter_label = ParameterLabel {
            name: Some("uniform_penalty_weight".to_string()),
            ..Default::default()
        };

        let mut quad_sum = Function::zero();
        let mut constraint_collection = self.constraint_collection;
        let mut removals = BTreeMap::new();
        for (&constraint_id, constraint) in constraint_collection.active() {
            let f = constraint.function().clone();
            let mut squared = f.clone();
            squared.try_mul_assign_in_place(&f)?;
            quad_sum.try_add_assign_in_place(squared)?;

            let removed_reason = crate::constraint::RemovedReason {
                reason: "ommx.Instance.uniform_penalty_method".to_string(),
                parameters: Default::default(),
            };
            removals.insert(constraint_id, (constraint.clone(), removed_reason));
        }
        constraint_collection.move_active_rows_to_removed(removals)?;

        let mut penalty_term = Function::from(linear!(parameter_id));
        penalty_term.try_mul_assign_in_place(&quad_sum)?;
        objective.try_add_assign_in_place(penalty_term)?;

        let mut parameters = ParameterTable::default();
        parameters.insert(parameter_id, parameter_label)?;

        Ok((
            ParametricInstance {
                sense: self.sense,
                objective,
                decision_variables: self.decision_variables,
                parameters,
                constraint_collection,
                indicator_constraint_collection: self.indicator_constraint_collection,
                one_hot_constraint_collection: self.one_hot_constraint_collection,
                sos1_constraint_collection: self.sos1_constraint_collection,
                decision_variable_dependency: self.decision_variable_dependency,
                description: self.description,
                named_functions: self.named_functions,
                annotations: self.annotations,
            },
            Some(parameter_id),
        ))
    }

    #[cfg_attr(doc, katexit::katexit)]
    /// Convert every active regular constraint to a penalty term with one fixed weight.
    ///
    /// Each constraint body $f_i(x)$ contributes `weight` $\cdot f_i(x)^2$,
    /// with the same mathematical and provenance semantics as
    /// [`Self::uniform_penalty_method`]. The generated penalty parameter is
    /// materialized immediately, so this operation mutates the [`Instance`] in
    /// place rather than returning a [`ParametricInstance`].
    ///
    /// When no active regular constraints exist, this is an identity
    /// operation. Otherwise, conversion and parameter materialization are
    /// performed on an internal clone and committed only after both succeed.
    ///
    /// # Errors
    ///
    /// Returns an error if [`Self::uniform_penalty_method`] or
    /// [`ParametricInstance::with_parameters`] fails. Any error leaves the
    /// instance unchanged.
    pub fn uniform_penalty_method_with_fixed_weight(&mut self, weight: f64) -> crate::Result<()> {
        self.ensure_penalty_method_supported("uniform_penalty_method")?;
        if self.constraints().is_empty() {
            return Ok(());
        }

        let (parametric, parameter_id) = self.clone().uniform_penalty_method_with_parameter_id()?;
        let parameter_id = parameter_id.expect("active constraints create one penalty parameter");
        self.commit_materialized_penalty(parametric, BTreeMap::from([(parameter_id, weight)]))
    }

    fn ensure_penalty_method_supported(&self, operation: &str) -> crate::Result<()> {
        anyhow::ensure!(
            self.indicator_constraint_collection.active().is_empty(),
            "{operation} does not support indicator constraints. \
             Remove or convert indicator constraints before applying penalty method."
        );
        anyhow::ensure!(
            self.one_hot_constraint_collection.active().is_empty(),
            "{operation} does not support one-hot constraints. \
             Remove or convert one-hot constraints before applying penalty method."
        );
        anyhow::ensure!(
            self.sos1_constraint_collection.active().is_empty(),
            "{operation} does not support SOS1 constraints. \
             Remove or convert SOS1 constraints before applying penalty method."
        );
        Ok(())
    }

    fn ensure_fixed_penalty_weight_ids(
        &self,
        weights: &BTreeMap<ConstraintID, f64>,
    ) -> crate::Result<()> {
        let active_ids = self.constraints().keys().copied().collect::<BTreeSet<_>>();
        let weight_ids = weights.keys().copied().collect::<BTreeSet<_>>();
        let missing_ids = active_ids
            .difference(&weight_ids)
            .copied()
            .collect::<Vec<_>>();
        let unexpected_ids = weight_ids
            .difference(&active_ids)
            .copied()
            .collect::<Vec<_>>();
        if !missing_ids.is_empty() || !unexpected_ids.is_empty() {
            crate::bail!(
                { ?missing_ids, ?unexpected_ids },
                "Fixed penalty weights must match active regular constraint IDs: \
                 missing {missing_ids:?}, unexpected {unexpected_ids:?}",
            );
        }
        Ok(())
    }

    fn commit_materialized_penalty(
        &mut self,
        parametric: ParametricInstance,
        parameter_values: BTreeMap<VariableID, f64>,
    ) -> crate::Result<()> {
        let parameters = crate::v1::Parameters {
            entries: parameter_values
                .into_iter()
                .map(|(id, value)| (id.into_inner(), value))
                .collect(),
        };
        let materialized = parametric.with_parameters(parameters)?;
        *self = materialized;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coeff, constraint::Equality, linear, quadratic, v1::State, ATol, ConstraintContext,
        DecisionVariable, Evaluate, ModelingLabel, Sense,
    };
    use std::collections::BTreeMap;

    /// Helper function to create a test instance with two decision variables and two constraints
    fn create_test_instance_with_constraints() -> Instance {
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(VariableID::from(1), DecisionVariable::continuous());
        decision_variables.insert(VariableID::from(2), DecisionVariable::continuous());

        let objective = Function::from((linear!(1) + linear!(2)).unwrap());

        let mut constraints = BTreeMap::new();
        constraints.insert(
            ConstraintID::from(1),
            Constraint {
                equality: Equality::LessThanOrEqualToZero,
                stage: crate::constraint::CreatedData {
                    function: Function::from(
                        ((linear!(1) + linear!(2)).unwrap() + coeff!(-1.0)).unwrap(),
                    ),
                },
            },
        );
        constraints.insert(
            ConstraintID::from(2),
            Constraint {
                equality: Equality::EqualToZero,
                stage: crate::constraint::CreatedData {
                    function: Function::from(linear!(1) + coeff!(-1.0) * linear!(2)),
                },
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
        assert_eq!(parametric_instance.constraints().len(), 0);
        assert_eq!(
            parametric_instance.removed_constraints().len(),
            original_constraint_count
        );

        // Check that correct number of parameters are created
        assert_eq!(parametric_instance.parameters().len(), expected_param_count);

        // Check parameter names
        for id in parametric_instance.parameters().keys() {
            assert_eq!(
                parametric_instance.parameters().labels().name(*id),
                Some(expected_param_name)
            );
        }

        // Verify ID separation
        let dv_ids: std::collections::BTreeSet<_> = parametric_instance
            .decision_variables
            .keys()
            .cloned()
            .collect();
        let p_ids: std::collections::BTreeSet<_> =
            parametric_instance.parameters().keys().cloned().collect();
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
        assert_eq!(substituted.constraints().len(), 0);
    }

    #[test]
    fn test_penalty_method() {
        let instance = create_test_instance_with_constraints();
        let original_objective = instance.objective.clone();
        let original_constraint_count = instance.constraints().len();
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
        let original_constraint_count = instance.constraints().len();
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
        decision_variables.insert(VariableID::from(1), DecisionVariable::continuous());

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
        assert_eq!(parametric_instance.constraints().len(), 0);
        assert_eq!(parametric_instance.removed_constraints().len(), 0);
        assert_eq!(parametric_instance.objective, objective);

        // Test uniform_penalty_method
        let parametric_instance = instance.uniform_penalty_method().unwrap();
        assert_eq!(parametric_instance.parameters.len(), 0);
        assert_eq!(parametric_instance.constraints().len(), 0);
        assert_eq!(parametric_instance.removed_constraints().len(), 0);
        assert_eq!(parametric_instance.objective, objective);
    }

    #[test]
    fn test_penalty_method_preserves_existing_removed_constraints() {
        let mut instance = create_test_instance_with_constraints();
        instance
            .set_constraint_context(
                ConstraintID::from(1),
                ConstraintContext {
                    label: ModelingLabel {
                        name: Some("already_removed".to_string()),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )
            .unwrap();
        instance
            .set_constraint_context(
                ConstraintID::from(2),
                ConstraintContext {
                    label: ModelingLabel {
                        name: Some("moved_by_penalty".to_string()),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )
            .unwrap();

        // Relax constraint 1 before applying penalty method
        instance
            .relax_constraint(
                ConstraintID::from(1),
                "pre_existing".to_string(),
                std::iter::empty::<(String, String)>(),
            )
            .unwrap();

        assert_eq!(instance.constraints().len(), 1); // only constraint 2 remains active
        assert_eq!(instance.removed_constraints().len(), 1); // constraint 1 is removed

        let parametric_instance = instance.penalty_method().unwrap();

        // Both constraints should be in removed: the pre-existing one and the newly penalized one
        assert_eq!(parametric_instance.removed_constraints().len(), 2);
        assert!(parametric_instance
            .removed_constraints()
            .contains_key(&ConstraintID::from(1)));
        assert!(parametric_instance
            .removed_constraints()
            .contains_key(&ConstraintID::from(2)));
        assert_eq!(
            parametric_instance
                .constraint_context()
                .name(ConstraintID::from(1)),
            Some("already_removed")
        );
        assert_eq!(
            parametric_instance
                .constraint_context()
                .name(ConstraintID::from(2)),
            Some("moved_by_penalty")
        );
        assert_eq!(
            parametric_instance.removed_constraints()[&ConstraintID::from(1)]
                .1
                .reason,
            "pre_existing"
        );
        assert_eq!(
            parametric_instance.removed_constraints()[&ConstraintID::from(2)]
                .1
                .reason,
            "ommx.Instance.penalty_method"
        );
    }

    #[test]
    fn test_uniform_penalty_method_preserves_existing_removed_constraints() {
        let mut instance = create_test_instance_with_constraints();
        instance
            .set_constraint_context(
                ConstraintID::from(1),
                ConstraintContext {
                    label: ModelingLabel {
                        name: Some("already_removed".to_string()),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )
            .unwrap();
        instance
            .set_constraint_context(
                ConstraintID::from(2),
                ConstraintContext {
                    label: ModelingLabel {
                        name: Some("moved_by_uniform_penalty".to_string()),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )
            .unwrap();

        // Relax constraint 1 before applying uniform penalty method
        instance
            .relax_constraint(
                ConstraintID::from(1),
                "pre_existing".to_string(),
                std::iter::empty::<(String, String)>(),
            )
            .unwrap();

        assert_eq!(instance.constraints().len(), 1);
        assert_eq!(instance.removed_constraints().len(), 1);

        let parametric_instance = instance.uniform_penalty_method().unwrap();

        // Both constraints should be in removed
        assert_eq!(parametric_instance.removed_constraints().len(), 2);
        assert!(parametric_instance
            .removed_constraints()
            .contains_key(&ConstraintID::from(1)));
        assert!(parametric_instance
            .removed_constraints()
            .contains_key(&ConstraintID::from(2)));
        assert_eq!(
            parametric_instance
                .constraint_context()
                .name(ConstraintID::from(1)),
            Some("already_removed")
        );
        assert_eq!(
            parametric_instance
                .constraint_context()
                .name(ConstraintID::from(2)),
            Some("moved_by_uniform_penalty")
        );
        assert_eq!(
            parametric_instance.removed_constraints()[&ConstraintID::from(1)]
                .1
                .reason,
            "pre_existing"
        );
        assert_eq!(
            parametric_instance.removed_constraints()[&ConstraintID::from(2)]
                .1
                .reason,
            "ommx.Instance.uniform_penalty_method"
        );
    }

    #[test]
    fn uniform_fixed_weight_penalty_is_materialized_in_place() {
        let mut instance = create_test_instance_with_constraints();

        instance
            .uniform_penalty_method_with_fixed_weight(2.0)
            .unwrap();

        assert!(instance.constraints().is_empty());
        assert_eq!(instance.removed_constraints().len(), 2);
        let state = State::from_iter([(1, 2.0), (2, 1.0)]);
        assert_eq!(
            instance
                .objective()
                .evaluate(&state, ATol::default())
                .unwrap(),
            13.0
        );

        let mut zero_weight = create_test_instance_with_constraints();
        let objective = zero_weight.objective().clone();
        zero_weight
            .uniform_penalty_method_with_fixed_weight(0.0)
            .unwrap();
        assert_eq!(
            zero_weight
                .objective()
                .evaluate(&state, ATol::default())
                .unwrap(),
            objective.evaluate(&state, ATol::default()).unwrap()
        );
        assert!(zero_weight.constraints().is_empty());
    }

    #[test]
    fn fixed_penalty_weights_bind_by_constraint_id() {
        let mut instance = create_test_instance_with_constraints();
        let weights = BTreeMap::from([(ConstraintID::from(1), 2.0), (ConstraintID::from(2), 3.0)]);

        instance
            .penalty_method_with_fixed_weights(&weights)
            .unwrap();

        assert!(instance.constraints().is_empty());
        assert_eq!(instance.removed_constraints().len(), 2);
        let state = State::from_iter([(1, 2.0), (2, 1.0)]);
        assert_eq!(
            instance
                .objective()
                .evaluate(&state, ATol::default())
                .unwrap(),
            14.0
        );
        let state = State::from_iter([(1, 0.0), (2, 0.0)]);
        assert_eq!(
            instance
                .objective()
                .evaluate(&state, ATol::default())
                .unwrap(),
            2.0
        );
    }

    #[test]
    fn fixed_weight_penalty_is_identity_without_active_constraints() {
        let mut instance = Instance::new(
            Sense::Minimize,
            Function::from(linear!(1)),
            BTreeMap::from([(VariableID::from(1), DecisionVariable::continuous())]),
            BTreeMap::new(),
        )
        .unwrap();
        let before = instance.clone();

        instance
            .penalty_method_with_fixed_weights(&BTreeMap::new())
            .unwrap();
        assert_eq!(instance, before);

        instance
            .uniform_penalty_method_with_fixed_weight(2.0)
            .unwrap();
        assert_eq!(instance, before);
    }

    #[test]
    fn fixed_penalty_weights_require_exact_active_id_coverage() {
        let before = create_test_instance_with_constraints();
        let cases = [
            BTreeMap::from([(ConstraintID::from(1), 2.0)]),
            BTreeMap::from([
                (ConstraintID::from(1), 2.0),
                (ConstraintID::from(2), 3.0),
                (ConstraintID::from(3), 4.0),
            ]),
        ];

        for weights in cases {
            let mut instance = before.clone();
            let err = instance
                .penalty_method_with_fixed_weights(&weights)
                .unwrap_err();

            assert!(err.to_string().contains("constraint IDs"));
            assert_eq!(instance, before);
        }
    }

    #[test]
    fn fixed_weight_penalty_materialization_failure_is_atomic() {
        let variable = VariableID::from(1);
        let make_instance = || {
            let objective =
                Function::Quadratic((coeff!(f64::MAX) * quadratic!(variable, variable)).unwrap());
            let constraint = Constraint {
                equality: Equality::EqualToZero,
                stage: crate::constraint::CreatedData {
                    function: Function::from(linear!(variable)),
                },
            };
            Instance::new(
                Sense::Minimize,
                objective,
                BTreeMap::from([(variable, DecisionVariable::continuous())]),
                BTreeMap::from([(ConstraintID::from(1), constraint)]),
            )
            .unwrap()
        };

        let mut uniform = make_instance();
        let before = uniform.clone();

        let err = uniform
            .uniform_penalty_method_with_fixed_weight(f64::MAX)
            .unwrap_err();
        assert!(matches!(
            err.downcast_ref::<crate::CoefficientError>(),
            Some(crate::CoefficientError::Infinite)
        ));
        assert_eq!(uniform, before);

        let mut keyed = make_instance();
        let before = keyed.clone();
        let err = keyed
            .penalty_method_with_fixed_weights(&BTreeMap::from([(ConstraintID::from(1), f64::MAX)]))
            .unwrap_err();
        assert!(matches!(
            err.downcast_ref::<crate::CoefficientError>(),
            Some(crate::CoefficientError::Infinite)
        ));
        assert_eq!(keyed, before);
    }
}
