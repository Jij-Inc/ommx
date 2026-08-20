use super::*;
use crate::{linear, ATol, Function, ParameterLabel, VariableID};
use anyhow::Result;
use std::collections::{BTreeMap, BTreeSet};

/// Signal returned when a fixed penalty weight is outside its numeric domain.
///
/// A fixed penalty weight must be finite and no smaller than the negative
/// absolute tolerance supplied to the owner operation. Callers can inspect the
/// rejected [`Self::weight`] and [`Self::atol`], revise the weight decision rule
/// or tolerance, and retry the unchanged [`Instance`].
#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
#[non_exhaustive]
#[error(
    "Fixed penalty weight must be finite and at least -atol: weight={weight}, atol={atol}",
    atol = .atol.into_inner()
)]
pub struct InvalidPenaltyWeight {
    weight: f64,
    atol: ATol,
}

impl InvalidPenaltyWeight {
    /// Return the rejected weight.
    pub fn weight(&self) -> f64 {
        self.weight
    }

    /// Return the absolute tolerance used to validate the weight.
    pub fn atol(&self) -> ATol {
        self.atol
    }
}

fn normalize_fixed_penalty_weight(weight: f64, atol: ATol) -> Result<f64, InvalidPenaltyWeight> {
    if !weight.is_finite() || weight < -atol {
        return Err(InvalidPenaltyWeight { weight, atol });
    }
    Ok(if weight < 0.0 { 0.0 } else { weight })
}

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
    ///
    /// When at least one constraint is converted, the pre-penalty active
    /// objective is preserved as the output objective. Because the penalty
    /// weights determine whether an optimum of the transformed formulation is
    /// also optimal for that output objective, optimality transport is marked
    /// as unavailable.
    pub fn penalty_method(mut self) -> Result<ParametricInstance> {
        self.ensure_penalty_method_supported("penalty_method")?;
        if !self.constraints().is_empty() {
            self.invalidate_output_objective_optimality();
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

        let id_base = max_id + 1;
        let mut objective = self.objective.clone();
        let mut parameters = ParameterTable::default();
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
            removals.insert(constraint_id, (constraint.clone(), removed_reason));
        }
        constraint_collection.move_active_rows_to_removed(removals)?;

        Ok(ParametricInstance {
            sense: self.sense,
            objective,
            output_objective: self.output_objective,
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
        })
    }

    #[cfg_attr(doc, katexit::katexit)]
    /// Convert every active regular constraint to a penalty term with its fixed weight.
    ///
    /// The keys of `weights` must be exactly the active regular constraint IDs.
    /// Each weight is a penalty magnitude: it must be finite and at least
    /// `-atol`. A weight in `[-atol, 0)` is normalized to zero. Constraint `id`
    /// with body $f_{id}(x)$ contributes $+w_{id} f_{id}(x)^2$ to a minimization
    /// objective and $-w_{id} f_{id}(x)^2$ to a maximization objective, where
    /// $w_{id}$ is the normalized nonnegative magnitude. OMMX does not decide
    /// whether a weight is large enough for an application's penalty rule.
    ///
    /// No parameter ID or assignment is created, and the instance's recorded
    /// parameter assignments remain exactly unchanged.
    ///
    /// On success, every regular, indicator, one-hot, and SOS1 constraint is in
    /// its removed collection; all four active collections are empty. Every
    /// active regular constraint is moved to the removed collection, and
    /// existing removed constraints of every family are preserved. Active
    /// special constraints are not penalty-converted: their presence returns
    /// an error without modifying the instance.
    ///
    /// When no constraint of any family is active, an empty `weights` map is an
    /// exact identity operation. Otherwise, fallible function arithmetic and
    /// regular-constraint lifecycle changes are completed on local values and
    /// committed only after both succeed.
    ///
    /// A successful non-identity conversion preserves the pre-penalty active
    /// objective as the output objective and marks optimality transport as
    /// unavailable.
    ///
    /// # Errors
    ///
    /// Returns an error if an unsupported active special constraint is present,
    /// if `weights` does not cover exactly the active regular constraints, if a
    /// weight is invalid, or if constructing or adding a penalty term fails.
    /// Invalid weights retain [`InvalidPenaltyWeight`] in the error chain. Any
    /// error leaves the instance unchanged.
    pub fn penalty_method_with_fixed_weights(
        &mut self,
        weights: &BTreeMap<ConstraintID, f64>,
        atol: ATol,
    ) -> crate::Result<()> {
        let operation = "penalty_method_with_fixed_weights";
        self.ensure_penalty_method_supported(operation)?;
        self.ensure_fixed_penalty_weight_ids(weights)?;
        if self.constraints().is_empty() {
            return Ok(());
        }

        let normalized_weights = weights
            .iter()
            .map(|(&id, &weight)| Ok((id, normalize_fixed_penalty_weight(weight, atol)?)))
            .collect::<Result<BTreeMap<_, _>, InvalidPenaltyWeight>>()?;

        let mut objective = self.objective.clone();
        for (&id, constraint) in self.constraint_collection.active() {
            let function = constraint.function();
            let mut penalty_term = function.clone();
            penalty_term.try_mul_assign_in_place(function)?;
            let weight = self.fixed_penalty_objective_coefficient(normalized_weights[&id]);
            penalty_term.try_mul_assign_in_place(&Function::try_from(weight)?)?;
            objective.try_add_assign_in_place(penalty_term)?;
        }
        self.commit_fixed_penalty(objective, operation)
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
    ///
    /// When at least one constraint is converted, the pre-penalty active
    /// objective is preserved as the output objective. Because the penalty
    /// weight determines whether an optimum of the transformed formulation is
    /// also optimal for that output objective, optimality transport is marked
    /// as unavailable.
    pub fn uniform_penalty_method(mut self) -> Result<ParametricInstance> {
        self.ensure_penalty_method_supported("uniform_penalty_method")?;

        // Early return if no active constraints (preserve any existing removed constraints)
        if self.constraints().is_empty() {
            return Ok(ParametricInstance {
                sense: self.sense,
                objective: self.objective,
                output_objective: self.output_objective,
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
            });
        }

        self.invalidate_output_objective_optimality();

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

        Ok(ParametricInstance {
            sense: self.sense,
            objective,
            output_objective: self.output_objective,
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
        })
    }

    #[cfg_attr(doc, katexit::katexit)]
    /// Convert every active regular constraint to a penalty term with one fixed weight.
    ///
    /// `weight` is a penalty magnitude: it must be finite and at least `-atol`.
    /// A value in `[-atol, 0)` is normalized to zero. Each constraint body
    /// $f_i(x)$ contributes $+w f_i(x)^2$ to a minimization objective and
    /// $-w f_i(x)^2$ to a maximization objective, where $w$ is the normalized
    /// nonnegative magnitude. OMMX does not decide whether the weight is large
    /// enough for an application's penalty rule.
    ///
    /// No parameter ID or assignment is created, and the instance's recorded
    /// parameter assignments remain exactly unchanged.
    ///
    /// On success, every regular, indicator, one-hot, and SOS1 constraint is in
    /// its removed collection; all four active collections are empty. Every
    /// active regular constraint is moved to the removed collection, and
    /// existing removed constraints of every family are preserved. Active
    /// special constraints are not penalty-converted: their presence returns
    /// an error without modifying the instance.
    ///
    /// When no constraint of any family is active, this is an exact identity
    /// operation. Otherwise, fallible function arithmetic and regular-constraint
    /// lifecycle changes are completed on local values and committed only after
    /// both succeed.
    ///
    /// A successful non-identity conversion preserves the pre-penalty active
    /// objective as the output objective and marks optimality transport as
    /// unavailable.
    ///
    /// # Errors
    ///
    /// Returns an error if an unsupported active special constraint is present,
    /// if `weight` is invalid, or if constructing or adding the penalty term
    /// fails. Invalid weights retain [`InvalidPenaltyWeight`] in the error
    /// chain. Any error leaves the instance unchanged.
    pub fn uniform_penalty_method_with_fixed_weight(
        &mut self,
        weight: f64,
        atol: ATol,
    ) -> crate::Result<()> {
        let operation = "uniform_penalty_method_with_fixed_weight";
        self.ensure_penalty_method_supported(operation)?;
        if self.constraints().is_empty() {
            return Ok(());
        }

        let weight = normalize_fixed_penalty_weight(weight, atol)?;
        let weight = self.fixed_penalty_objective_coefficient(weight);

        let mut penalty_term = Function::zero();
        for constraint in self.constraint_collection.active().values() {
            let function = constraint.function();
            let mut squared = function.clone();
            squared.try_mul_assign_in_place(function)?;
            penalty_term.try_add_assign_in_place(squared)?;
        }
        penalty_term.try_mul_assign_in_place(&Function::try_from(weight)?)?;

        let mut objective = self.objective.clone();
        objective.try_add_assign_in_place(penalty_term)?;
        self.commit_fixed_penalty(objective, operation)
    }

    fn fixed_penalty_objective_coefficient(&self, weight: f64) -> f64 {
        match self.sense() {
            Sense::Minimize => weight,
            Sense::Maximize => -weight,
        }
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

    fn commit_fixed_penalty(&mut self, objective: Function, operation: &str) -> crate::Result<()> {
        let reason = format!("ommx.Instance.{operation}");
        let removals = self
            .constraint_collection
            .active()
            .iter()
            .map(|(&id, constraint)| {
                (
                    id,
                    (
                        constraint.clone(),
                        crate::constraint::RemovedReason {
                            reason: reason.clone(),
                            parameters: Default::default(),
                        },
                    ),
                )
            })
            .collect();
        let mut constraint_collection = self.constraint_collection.clone();
        constraint_collection.move_active_rows_to_removed(removals)?;

        if !self.constraint_collection.active().is_empty() || objective != self.objective {
            self.invalidate_output_objective_optimality();
        }
        self.objective = objective;
        self.constraint_collection = constraint_collection;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coeff, constraint::Equality, linear, quadratic, v1::State, ATol, ConstraintContext,
        DecisionVariable, Evaluate, ModelingLabel, Sampled, Sense,
    };
    use std::collections::{BTreeMap, BTreeSet};

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum SpecialConstraintKind {
        Indicator,
        OneHot,
        Sos1,
    }

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

    fn create_test_instance_with_special_constraints(kinds: &[SpecialConstraintKind]) -> Instance {
        let variable = VariableID::from(1);
        let regular_constraint = Constraint::equal_to_zero(Function::from(linear!(variable)));
        let has_kind = |kind| kinds.contains(&kind);

        let indicator_constraints = has_kind(SpecialConstraintKind::Indicator)
            .then(|| {
                (
                    crate::IndicatorConstraintID::from(1),
                    crate::IndicatorConstraint::new(
                        variable,
                        Equality::EqualToZero,
                        Function::Zero,
                    ),
                )
            })
            .into_iter()
            .collect();
        let one_hot_constraints = has_kind(SpecialConstraintKind::OneHot)
            .then(|| {
                (
                    crate::OneHotConstraintID::from(1),
                    crate::OneHotConstraint::new(BTreeSet::from([variable])).unwrap(),
                )
            })
            .into_iter()
            .collect();
        let sos1_constraints = has_kind(SpecialConstraintKind::Sos1)
            .then(|| {
                (
                    crate::Sos1ConstraintID::from(1),
                    crate::Sos1Constraint::new(BTreeSet::from([variable])).unwrap(),
                )
            })
            .into_iter()
            .collect();

        Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::from([(variable, DecisionVariable::binary())]))
            .constraints(BTreeMap::from([(
                ConstraintID::from(1),
                regular_constraint,
            )]))
            .indicator_constraints(indicator_constraints)
            .one_hot_constraints(one_hot_constraints)
            .sos1_constraints(sos1_constraints)
            .build()
            .unwrap()
    }

    fn assert_no_active_constraints(instance: &Instance) {
        assert!(instance.constraints().is_empty());
        assert!(instance.indicator_constraints().is_empty());
        assert!(instance.one_hot_constraints().is_empty());
        assert!(instance.sos1_constraints().is_empty());
    }

    fn assert_fixed_penalty_removal_provenance(instance: &Instance, operation: &str) {
        let expected_reason = format!("ommx.Instance.{operation}");
        for (_, reason) in instance.removed_constraints().values() {
            assert_eq!(reason.reason, expected_reason);
            assert!(reason.parameters.is_empty());
        }
    }

    fn assert_penalty_output_semantics(
        instance: &Instance,
        expected_active: f64,
        expected_output: f64,
    ) {
        let state = State::from_iter([(1, 2.0), (2, 1.0)]);
        let output = instance.output_objective().unwrap();
        assert_eq!(output.sense(), Sense::Minimize);
        assert!(!output.preserves_optimality());
        assert_eq!(
            output.function().evaluate(&state, ATol::default()).unwrap(),
            expected_output
        );
        assert_eq!(
            instance
                .objective()
                .evaluate(&state, ATol::default())
                .unwrap(),
            expected_active
        );

        let solution = instance.evaluate(&state, ATol::default()).unwrap();
        assert_eq!(*solution.sense(), Some(Sense::Minimize));
        assert_eq!(*solution.objective(), expected_output);

        let sample_set = instance
            .evaluate_samples(&Sampled::from(state), ATol::default())
            .unwrap();
        let sample_id = sample_set.sample_ids().into_iter().next().unwrap();
        assert_eq!(*sample_set.sense(), Sense::Minimize);
        assert_eq!(
            sample_set.objectives().get(sample_id),
            Some(&expected_output)
        );

        assert!(instance.to_v1_bytes().is_err());
        let restored = Instance::from_v2_bytes(&instance.to_v2_bytes()).unwrap();
        assert_eq!(restored.output_objective(), instance.output_objective());
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

        let output = parametric_instance.output_objective().unwrap();
        assert_eq!(output.sense(), parametric_instance.sense);
        assert_eq!(output.function(), &original_objective);
        assert!(!output.preserves_optimality());
        assert!(parametric_instance.to_v1_bytes().is_err());
        let restored =
            ParametricInstance::from_v2_bytes(&parametric_instance.to_v2_bytes()).unwrap();
        assert_eq!(
            restored.output_objective(),
            parametric_instance.output_objective()
        );

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
        let output = substituted.output_objective().unwrap();
        assert_eq!(output.sense(), substituted.sense);
        assert_eq!(output.function(), &original_objective);
        assert!(!output.preserves_optimality());
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
    fn direct_parametric_penalty_preserves_the_original_objective_for_output() {
        for parametric in [
            create_test_instance_with_constraints()
                .penalty_method()
                .unwrap(),
            create_test_instance_with_constraints()
                .uniform_penalty_method()
                .unwrap(),
        ] {
            let parameters = crate::v1::Parameters {
                entries: parametric
                    .parameters()
                    .keys()
                    .map(|id| (id.into_inner(), 2.0))
                    .collect(),
            };
            let instance = parametric.with_parameters(parameters).unwrap();
            assert_penalty_output_semantics(&instance, 13.0, 3.0);
        }
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
        assert!(parametric_instance.output_objective().is_none());

        // Test uniform_penalty_method
        let parametric_instance = instance.uniform_penalty_method().unwrap();
        assert_eq!(parametric_instance.parameters.len(), 0);
        assert_eq!(parametric_instance.constraints().len(), 0);
        assert_eq!(parametric_instance.removed_constraints().len(), 0);
        assert_eq!(parametric_instance.objective, objective);
        assert!(parametric_instance.output_objective().is_none());
    }

    #[test]
    fn no_constraint_penalty_preserves_existing_output_objective() {
        let mut instance = Instance::new(
            Sense::Minimize,
            Function::from(linear!(1)),
            BTreeMap::from([(VariableID::from(1), DecisionVariable::continuous())]),
            BTreeMap::new(),
        )
        .unwrap();
        assert!(instance.convert_active_objective(Sense::Maximize));
        let output = instance.output_objective().cloned().unwrap();

        let keyed = instance.clone().penalty_method().unwrap();
        assert_eq!(keyed.output_objective(), Some(&output));

        let uniform = instance.uniform_penalty_method().unwrap();
        assert_eq!(uniform.output_objective(), Some(&output));
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
    fn uniform_fixed_weight_penalty_updates_objective_in_place() {
        let mut instance = create_test_instance_with_constraints();

        instance
            .uniform_penalty_method_with_fixed_weight(2.0, ATol::default())
            .unwrap();

        assert_no_active_constraints(&instance);
        assert_eq!(instance.removed_constraints().len(), 2);
        assert!(instance.parameters.is_none());
        assert_fixed_penalty_removal_provenance(
            &instance,
            "uniform_penalty_method_with_fixed_weight",
        );
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
            .uniform_penalty_method_with_fixed_weight(0.0, ATol::default())
            .unwrap();
        assert_eq!(
            zero_weight
                .objective()
                .evaluate(&state, ATol::default())
                .unwrap(),
            objective.evaluate(&state, ATol::default()).unwrap()
        );
        assert_no_active_constraints(&zero_weight);
    }

    #[test]
    fn fixed_penalty_direction_follows_instance_sense() {
        let state = State::from_iter([(1, 2.0), (2, 1.0)]);

        let mut uniform = create_test_instance_with_constraints();
        uniform.sense = Sense::Maximize;
        let uniform_original = uniform.objective().clone();
        uniform
            .uniform_penalty_method_with_fixed_weight(2.0, ATol::default())
            .unwrap();
        assert_eq!(
            uniform
                .objective()
                .evaluate(&state, ATol::default())
                .unwrap(),
            -7.0
        );
        let uniform_output = uniform.output_objective().unwrap();
        assert_eq!(uniform_output.sense(), Sense::Maximize);
        assert_eq!(uniform_output.function(), &uniform_original);
        assert!(!uniform_output.preserves_optimality());

        let mut keyed = create_test_instance_with_constraints();
        keyed.sense = Sense::Maximize;
        let keyed_original = keyed.objective().clone();
        keyed
            .penalty_method_with_fixed_weights(
                &BTreeMap::from([(ConstraintID::from(1), 2.0), (ConstraintID::from(2), 3.0)]),
                ATol::default(),
            )
            .unwrap();
        assert_eq!(
            keyed.objective().evaluate(&state, ATol::default()).unwrap(),
            -8.0
        );
        let keyed_output = keyed.output_objective().unwrap();
        assert_eq!(keyed_output.sense(), Sense::Maximize);
        assert_eq!(keyed_output.function(), &keyed_original);
        assert!(!keyed_output.preserves_optimality());
    }

    #[test]
    fn fixed_penalty_preserves_existing_output_objective() {
        let mut instance = create_test_instance_with_constraints();
        instance.sense = Sense::Maximize;
        let original_objective = instance.objective().clone();
        assert!(instance.convert_active_objective(Sense::Minimize));

        instance
            .uniform_penalty_method_with_fixed_weight(2.0, ATol::default())
            .unwrap();

        let output = instance.output_objective().unwrap();
        assert_eq!(output.sense(), Sense::Maximize);
        assert_eq!(output.function(), &original_objective);
        assert!(!output.preserves_optimality());
        let restored = Instance::from_v2_bytes(&instance.to_v2_bytes()).unwrap();
        assert!(!restored.output_objective().unwrap().preserves_optimality());
        let solution = instance
            .evaluate(&State::from_iter([(1, 2.0), (2, 1.0)]), ATol::default())
            .unwrap();
        assert_eq!(*solution.sense(), Some(Sense::Maximize));
        assert_eq!(*solution.objective(), 3.0);
    }

    #[test]
    fn direct_fixed_penalty_preserves_the_original_objective_even_at_zero_weight() {
        let mut instance = create_test_instance_with_constraints();
        let original_objective = instance.objective().clone();

        instance
            .uniform_penalty_method_with_fixed_weight(0.0, ATol::default())
            .unwrap();

        let output = instance.output_objective().unwrap();
        assert_eq!(output.sense(), Sense::Minimize);
        assert_eq!(output.function(), &original_objective);
        assert!(!output.preserves_optimality());
    }

    #[test]
    fn direct_fixed_penalty_preserves_the_original_objective_for_output() {
        let mut instance = create_test_instance_with_constraints();
        instance
            .uniform_penalty_method_with_fixed_weight(2.0, ATol::default())
            .unwrap();

        assert_penalty_output_semantics(&instance, 13.0, 3.0);
    }

    #[test]
    fn parametric_penalty_methods_preserve_existing_output_objective() {
        let mut instance = create_test_instance_with_constraints();
        instance.sense = Sense::Maximize;
        let original_objective = instance.objective().clone();
        assert!(instance.convert_active_objective(Sense::Minimize));

        for parametric in [
            instance.clone().penalty_method().unwrap(),
            instance.uniform_penalty_method().unwrap(),
        ] {
            let output = parametric.output_objective().unwrap();
            assert_eq!(output.sense(), Sense::Maximize);
            assert_eq!(output.function(), &original_objective);
            assert!(!output.preserves_optimality());

            let parameters = crate::v1::Parameters {
                entries: parametric
                    .parameters()
                    .keys()
                    .map(|id| (id.into_inner(), 2.0))
                    .collect(),
            };
            let materialized = parametric.with_parameters(parameters).unwrap();
            let solution = materialized
                .evaluate(&State::from_iter([(1, 2.0), (2, 1.0)]), ATol::default())
                .unwrap();
            assert_eq!(*solution.sense(), Some(Sense::Maximize));
            assert_eq!(*solution.objective(), 3.0);
        }
    }

    #[test]
    fn fixed_penalty_normalizes_tolerated_negative_weights_to_zero() {
        let atol = ATol::new(0.1).unwrap();
        let state = State::from_iter([(1, 2.0), (2, 1.0)]);

        let mut uniform = create_test_instance_with_constraints();
        let uniform_objective = uniform.objective().clone();
        uniform
            .uniform_penalty_method_with_fixed_weight(-0.1, atol)
            .unwrap();
        assert_eq!(
            uniform
                .objective()
                .evaluate(&state, ATol::default())
                .unwrap(),
            uniform_objective.evaluate(&state, ATol::default()).unwrap()
        );
        assert_no_active_constraints(&uniform);

        let mut keyed = create_test_instance_with_constraints();
        let keyed_objective = keyed.objective().clone();
        keyed
            .penalty_method_with_fixed_weights(
                &BTreeMap::from([
                    (ConstraintID::from(1), -0.1),
                    (ConstraintID::from(2), -0.05),
                ]),
                atol,
            )
            .unwrap();
        assert_eq!(
            keyed.objective().evaluate(&state, ATol::default()).unwrap(),
            keyed_objective.evaluate(&state, ATol::default()).unwrap()
        );
        assert_no_active_constraints(&keyed);
    }

    #[test]
    fn fixed_penalty_rejects_weights_below_tolerance_atomically() {
        let atol = ATol::new(0.1).unwrap();
        let before = create_test_instance_with_constraints();

        let mut uniform = before.clone();
        let error = uniform
            .uniform_penalty_method_with_fixed_weight(-0.100_001, atol)
            .unwrap_err();
        let signal = error.downcast_ref::<InvalidPenaltyWeight>().unwrap();
        assert_eq!(signal.weight(), -0.100_001);
        assert_eq!(signal.atol(), atol);
        assert_eq!(uniform, before);

        let mut keyed = before.clone();
        let error = keyed
            .penalty_method_with_fixed_weights(
                &BTreeMap::from([
                    (ConstraintID::from(1), 2.0),
                    (ConstraintID::from(2), -0.100_001),
                ]),
                atol,
            )
            .unwrap_err();
        assert!(error.is::<InvalidPenaltyWeight>());
        assert_eq!(keyed, before);
    }

    #[test]
    fn fixed_penalty_rejects_non_finite_weights_atomically() {
        for weight in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let before = create_test_instance_with_constraints();

            let mut uniform = before.clone();
            let error = uniform
                .uniform_penalty_method_with_fixed_weight(weight, ATol::default())
                .unwrap_err();
            assert!(error.is::<InvalidPenaltyWeight>());
            assert_eq!(uniform, before);

            let mut keyed = before.clone();
            let error = keyed
                .penalty_method_with_fixed_weights(
                    &BTreeMap::from([
                        (ConstraintID::from(1), 2.0),
                        (ConstraintID::from(2), weight),
                    ]),
                    ATol::default(),
                )
                .unwrap_err();
            assert!(error.is::<InvalidPenaltyWeight>());
            assert_eq!(keyed, before);
        }
    }

    #[test]
    fn fixed_penalty_weights_bind_by_constraint_id() {
        let mut instance = create_test_instance_with_constraints();
        instance
            .set_constraint_context(
                ConstraintID::from(1),
                ConstraintContext {
                    label: ModelingLabel {
                        name: Some("penalized".to_string()),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )
            .unwrap();
        let weights = BTreeMap::from([(ConstraintID::from(1), 2.0), (ConstraintID::from(2), 3.0)]);

        instance
            .penalty_method_with_fixed_weights(&weights, ATol::default())
            .unwrap();

        assert_no_active_constraints(&instance);
        assert_eq!(instance.removed_constraints().len(), 2);
        assert!(instance.parameters.is_none());
        assert_eq!(
            instance.constraint_context().name(ConstraintID::from(1)),
            Some("penalized")
        );
        assert_fixed_penalty_removal_provenance(&instance, "penalty_method_with_fixed_weights");
        assert_penalty_output_semantics(&instance, 14.0, 3.0);
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
            .penalty_method_with_fixed_weights(&BTreeMap::new(), ATol::default())
            .unwrap();
        assert_eq!(instance, before);

        instance
            .uniform_penalty_method_with_fixed_weight(2.0, ATol::default())
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
                .penalty_method_with_fixed_weights(&weights, ATol::default())
                .unwrap_err();

            assert!(err.to_string().contains("constraint IDs"));
            assert_eq!(instance, before);
        }
    }

    #[test]
    fn fixed_weight_penalty_rejects_each_active_special_constraint_atomically() {
        for kind in [
            SpecialConstraintKind::Indicator,
            SpecialConstraintKind::OneHot,
            SpecialConstraintKind::Sos1,
        ] {
            let before = create_test_instance_with_special_constraints(&[kind]);
            let weights = before.constraints().keys().map(|id| (*id, 2.0)).collect();

            let mut keyed = before.clone();
            keyed
                .penalty_method_with_fixed_weights(&weights, ATol::default())
                .unwrap_err();
            assert_eq!(keyed, before);

            let mut uniform = before.clone();
            uniform
                .uniform_penalty_method_with_fixed_weight(2.0, ATol::default())
                .unwrap_err();
            assert_eq!(uniform, before);
        }
    }

    #[test]
    fn fixed_weight_penalty_success_leaves_every_constraint_family_removed() {
        let mut lowered = create_test_instance_with_special_constraints(&[
            SpecialConstraintKind::Indicator,
            SpecialConstraintKind::OneHot,
            SpecialConstraintKind::Sos1,
        ]);
        lowered
            .convert_indicator_to_constraint(crate::IndicatorConstraintID::from(1))
            .unwrap();
        lowered
            .convert_one_hot_to_constraint(crate::OneHotConstraintID::from(1))
            .unwrap();
        lowered
            .convert_sos1_to_constraints(crate::Sos1ConstraintID::from(1))
            .unwrap();

        assert_eq!(lowered.removed_indicator_constraints().len(), 1);
        assert_eq!(lowered.removed_one_hot_constraints().len(), 1);
        assert_eq!(lowered.removed_sos1_constraints().len(), 1);
        assert!(!lowered.constraints().is_empty());

        let already_removed_regular_id = ConstraintID::from(1);
        lowered
            .set_constraint_context(
                already_removed_regular_id,
                ConstraintContext {
                    label: ModelingLabel {
                        name: Some("already_removed_regular".to_string()),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )
            .unwrap();
        lowered
            .relax_constraint(
                already_removed_regular_id,
                "pre_existing".to_string(),
                std::iter::empty::<(String, String)>(),
            )
            .unwrap();

        let regular_constraint_count =
            lowered.constraints().len() + lowered.removed_constraints().len();
        let already_removed_regular =
            lowered.removed_constraints()[&already_removed_regular_id].clone();
        let removed_indicators = lowered.removed_indicator_constraints().clone();
        let removed_one_hots = lowered.removed_one_hot_constraints().clone();
        let removed_sos1s = lowered.removed_sos1_constraints().clone();

        let mut keyed = lowered.clone();
        let weights = keyed.constraints().keys().map(|id| (*id, 2.0)).collect();
        keyed
            .penalty_method_with_fixed_weights(&weights, ATol::default())
            .unwrap();
        assert_no_active_constraints(&keyed);
        assert_eq!(keyed.removed_constraints().len(), regular_constraint_count);
        assert_eq!(
            keyed.removed_constraints()[&already_removed_regular_id],
            already_removed_regular
        );
        assert_eq!(
            keyed.constraint_context().name(already_removed_regular_id),
            Some("already_removed_regular")
        );
        assert_eq!(keyed.removed_indicator_constraints(), &removed_indicators);
        assert_eq!(keyed.removed_one_hot_constraints(), &removed_one_hots);
        assert_eq!(keyed.removed_sos1_constraints(), &removed_sos1s);

        let mut uniform = lowered;
        uniform
            .uniform_penalty_method_with_fixed_weight(2.0, ATol::default())
            .unwrap();
        assert_no_active_constraints(&uniform);
        assert_eq!(
            uniform.removed_constraints().len(),
            regular_constraint_count
        );
        assert_eq!(
            uniform.removed_constraints()[&already_removed_regular_id],
            already_removed_regular
        );
        assert_eq!(
            uniform
                .constraint_context()
                .name(already_removed_regular_id),
            Some("already_removed_regular")
        );
        assert_eq!(uniform.removed_indicator_constraints(), &removed_indicators);
        assert_eq!(uniform.removed_one_hot_constraints(), &removed_one_hots);
        assert_eq!(uniform.removed_sos1_constraints(), &removed_sos1s);
    }

    #[test]
    fn fixed_weight_penalty_preserves_parameters_exactly() {
        let parameters = Some(crate::v1::Parameters {
            entries: [(100, 3.0), (200, -4.0)].into_iter().collect(),
        });

        let mut keyed = create_test_instance_with_constraints();
        keyed.parameters = parameters.clone();
        keyed
            .penalty_method_with_fixed_weights(
                &BTreeMap::from([(ConstraintID::from(1), 2.0), (ConstraintID::from(2), 3.0)]),
                ATol::default(),
            )
            .unwrap();
        assert_eq!(keyed.parameters, parameters);

        let mut uniform = create_test_instance_with_constraints();
        uniform.parameters = parameters.clone();
        uniform
            .uniform_penalty_method_with_fixed_weight(2.0, ATol::default())
            .unwrap();
        assert_eq!(uniform.parameters, parameters);
    }

    #[test]
    fn fixed_weight_penalty_arithmetic_failure_is_atomic() {
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
            .uniform_penalty_method_with_fixed_weight(f64::MAX, ATol::default())
            .unwrap_err();
        assert!(matches!(
            err.downcast_ref::<crate::CoefficientError>(),
            Some(crate::CoefficientError::Infinite)
        ));
        assert_eq!(uniform, before);

        let mut keyed = make_instance();
        let before = keyed.clone();
        let err = keyed
            .penalty_method_with_fixed_weights(
                &BTreeMap::from([(ConstraintID::from(1), f64::MAX)]),
                ATol::default(),
            )
            .unwrap_err();
        assert!(matches!(
            err.downcast_ref::<crate::CoefficientError>(),
            Some(crate::CoefficientError::Infinite)
        ));
        assert_eq!(keyed, before);
    }
}
