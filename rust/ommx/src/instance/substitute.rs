use super::*;
use crate::{
    substitute_acyclic, substitute_one_via_acyclic, ATol, DecisionVariable, Function,
    IndicatorConstraintID, ModelingLabel, Substitute, SubstitutionError, VariableID,
};
use std::collections::{BTreeMap, BTreeSet};

/// Planned expression replacements in an [`Instance`] Active Formulation.
///
/// The root object prepares this plan without mutation, then commits only
/// table-local replacements. This keeps substitution atomic without granting
/// crate-wide access to a partially mutating `Instance` operation.
struct InstanceSubstitutionPlan {
    objective: Option<Function>,
    constraint_replacements: BTreeMap<ConstraintID, Constraint>,
    indicator_replacements: BTreeMap<IndicatorConstraintID, IndicatorConstraint>,
}

impl Instance {
    /// Plan the Active Formulation rewrite; reporting expressions use reconstructed values.
    fn plan_substitution(
        &self,
        acyclic: &crate::AcyclicAssignments,
    ) -> Result<InstanceSubstitutionPlan, crate::SubstitutionError> {
        debug_assert!(!acyclic.is_empty());
        let substituted_variables: BTreeSet<VariableID> =
            acyclic.iter().map(|(var_id, _)| *var_id).collect();

        for &variable in &substituted_variables {
            if self.fixed_decision_variable_value(variable).is_some()
                || self.decision_variable_dependency.get(&variable).is_some()
            {
                return Err(SubstitutionError::AssignedVariableSubstitution { variable });
            }
        }

        // Structural special-constraint variables cannot be substituted. Check
        // every family while the operation is still read-only.
        for (&constraint_id, constraint) in self.indicator_constraint_collection.active() {
            if substituted_variables.contains(&constraint.indicator_variable) {
                return Err(SubstitutionError::IndicatorVariableSubstitution {
                    indicator_variable: constraint.indicator_variable,
                    constraint_id,
                });
            }
        }
        for (&constraint_id, constraint) in self.one_hot_constraint_collection.active() {
            for &variable in &constraint.variables {
                if substituted_variables.contains(&variable) {
                    return Err(SubstitutionError::OneHotVariableSubstitution {
                        variable,
                        constraint_id,
                    });
                }
            }
        }
        for (&constraint_id, constraint) in self.sos1_constraint_collection.active() {
            for &variable in &constraint.variables {
                if substituted_variables.contains(&variable) {
                    return Err(SubstitutionError::Sos1VariableSubstitution {
                        variable,
                        constraint_id,
                    });
                }
            }
        }

        let objective = if self
            .objective
            .required_ids()
            .is_disjoint(&substituted_variables)
        {
            None
        } else {
            Some(self.objective.clone().substitute_acyclic(acyclic)?)
        };

        // Inactive constraints are intentionally not rewritten here; they are
        // normalized through `activate_constraint` when reactivated.
        let mut constraint_replacements = BTreeMap::new();
        for (&constraint_id, constraint) in self.constraint_collection.active() {
            if !constraint
                .required_ids()
                .is_disjoint(&substituted_variables)
            {
                let mut replacement = constraint.clone();
                replacement.stage.function =
                    replacement.stage.function.substitute_acyclic(acyclic)?;
                constraint_replacements.insert(constraint_id, replacement);
            }
        }

        let mut indicator_replacements = BTreeMap::new();
        for (&constraint_id, constraint) in self.indicator_constraint_collection.active() {
            if !constraint
                .stage
                .function
                .required_ids()
                .is_disjoint(&substituted_variables)
            {
                let mut replacement = constraint.clone();
                replacement.stage.function =
                    replacement.stage.function.substitute_acyclic(acyclic)?;
                indicator_replacements.insert(constraint_id, replacement);
            }
        }

        Ok(InstanceSubstitutionPlan {
            objective,
            constraint_replacements,
            indicator_replacements,
        })
    }

    /// Commit a fully validated substitution plan using table-local effects.
    fn commit_substitution(&mut self, plan: InstanceSubstitutionPlan) {
        if let Some(objective) = plan.objective {
            self.replace_active_objective_preserving_output(objective);
        }
        self.constraint_collection
            .replace_active_rows(plan.constraint_replacements)
            .expect("replacement IDs were read from active constraints");
        self.indicator_constraint_collection
            .replace_active_rows(plan.indicator_replacements)
            .expect("replacement IDs were read from active indicator constraints");
    }

    /// Apply an exact encoding's active-formulation substitution together with
    /// prevalidated fresh decision variables.
    ///
    /// This operation captures the pre-encoding active objective if and only if
    /// the validated substitution targets it. Capture happens after the last
    /// fallible operation, immediately before the planned rewrite is committed.
    /// Whether the rewritten function happens to equal the original function is
    /// irrelevant: ownership of the rewrite determines the capture boundary.
    pub(super) fn apply_encoding_substitution(
        &mut self,
        acyclic: &crate::AcyclicAssignments,
        fresh_variables: Vec<(VariableID, DecisionVariable, ModelingLabel)>,
        atol: ATol,
    ) -> crate::Result<()> {
        let mut fresh_ids = BTreeSet::new();
        for (id, _, _) in &fresh_variables {
            if self.decision_variables.contains_key(id) || !fresh_ids.insert(*id) {
                crate::bail!({ ?id }, "Fresh decision variable ID is already owned: {id:?}");
            }
        }
        for id in acyclic.keys() {
            if !self.decision_variables.contains_key(&id) {
                crate::bail!({ ?id }, "Substitution target is not owned by this instance: {id:?}");
            }
        }
        for id in acyclic.required_ids() {
            if !self.decision_variables.contains_key(&id) && !fresh_ids.contains(&id) {
                crate::bail!(
                    { ?id },
                    "Substitution references an unowned decision variable: {id:?}",
                );
            }
        }

        let plan = self.plan_substitution(acyclic)?;

        // This is the last fallible operation. It plans and validates the
        // dependency update before mutating the table, and clones only the
        // assignment functions affected by this substitution.
        self.decision_variable_dependency
            .substitute_acyclic_in_place_atomic(acyclic)?;

        for (id, variable, label) in fresh_variables {
            self.decision_variables
                .insert(id, variable, label, None, atol)
                .expect("fresh decision variable IDs were reserved from this instance");
        }
        self.commit_substitution(plan);
        Ok(())
    }
}

/// # Postconditions
///
/// Substitution rewrites the Active Formulation and composes reconstruction
/// rules. Named functions, inactive constraints, and an existing output objective
/// retain their source expressions. If the active objective is affected and no
/// output objective exists, its previous function and sense are retained.
/// Evaluation reconstructs the substituted variables before evaluating these
/// expressions. This does not establish equivalence of optimization problems;
/// see [`Substitute`] for the caller's control over domain preservation.
/// Fixed and dependent variables cannot be substitution targets. To extend
/// a representation, substitute its remaining independent variables.
///
/// ```
/// use ommx::{
///     linear, substitute, v1::State, ATol, DecisionVariable, Evaluate, Function,
///     Instance, Sense, VariableID,
/// };
/// use std::collections::{BTreeMap, HashMap};
///
/// let original = VariableID::from(0);
/// let replacement = VariableID::from(1);
/// let mut instance = Instance::builder()
///     .sense(Sense::Maximize)
///     .objective(Function::from(linear!(0)))
///     .decision_variables(BTreeMap::from([
///         (original, DecisionVariable::binary()),
///         (replacement, DecisionVariable::binary()),
///     ]))
///     .active_constraints(BTreeMap::new())
///     .build()
///     .unwrap();
/// assert!(instance.convert_active_objective(Sense::Minimize));
/// let output = instance.output_objective().cloned();
///
/// substitute(&mut instance, [(original, Function::from(linear!(1)))]).unwrap();
/// assert_eq!(instance.output_objective(), output.as_ref());
/// let state = State::from(HashMap::from([(1, 1.0)]));
/// assert_eq!(instance.objective().evaluate(&state, ATol::default()).unwrap(), -1.0);
/// let solution = instance.evaluate(&state, ATol::default()).unwrap();
/// assert_eq!(*solution.sense(), Some(Sense::Maximize));
/// assert_eq!(*solution.objective(), 1.0);
/// ```
impl Substitute for Instance {
    type Output = Self;

    fn substitute_acyclic(
        mut self,
        acyclic: &crate::AcyclicAssignments,
    ) -> Result<Self::Output, crate::SubstitutionError> {
        if acyclic.is_empty() {
            return Ok(self);
        }
        let plan = self.plan_substitution(acyclic)?;
        self.decision_variable_dependency =
            std::mem::take(&mut self.decision_variable_dependency).substitute_acyclic(acyclic)?;
        self.commit_substitution(plan);
        Ok(self)
    }

    fn substitute_one(
        self,
        assigned: VariableID,
        f: &Function,
    ) -> Result<Self::Output, SubstitutionError> {
        substitute_one_via_acyclic(self, assigned, f)
    }
}

impl Substitute for ParametricInstance {
    type Output = Self;

    fn substitute_acyclic(
        mut self,
        acyclic: &crate::AcyclicAssignments,
    ) -> Result<Self::Output, crate::SubstitutionError> {
        if acyclic.is_empty() {
            return Ok(self);
        }

        let substituted_variables: std::collections::BTreeSet<VariableID> =
            acyclic.iter().map(|(var_id, _)| *var_id).collect();

        for (var_id, function) in acyclic.iter() {
            if self.parameters.contains_key(var_id) {
                return Err(SubstitutionError::ParameterSubstitution { parameter: *var_id });
            }
            if !self.decision_variables.contains_key(var_id) {
                return Err(SubstitutionError::UndefinedSubstitutionVariable { variable: *var_id });
            }
            if self.decision_variables.fixed_value(*var_id).is_some()
                || self.decision_variable_dependency.get(var_id).is_some()
            {
                return Err(SubstitutionError::AssignedVariableSubstitution { variable: *var_id });
            }
            for required_id in function.required_ids() {
                if !self.decision_variables.contains_key(&required_id)
                    && !self.parameters.contains_key(&required_id)
                {
                    return Err(SubstitutionError::UndefinedSubstitutionVariable {
                        variable: required_id,
                    });
                }
            }
        }

        let mut affected_constraint_ids = std::collections::BTreeSet::new();
        for (constraint_id, constraint) in self.constraint_collection.active() {
            let required_ids = constraint.required_ids();
            if !required_ids.is_disjoint(&substituted_variables) {
                affected_constraint_ids.insert(*constraint_id);
            }
        }

        if !self
            .objective
            .required_ids()
            .is_disjoint(&substituted_variables)
        {
            let replacement = self.objective.clone().substitute_acyclic(acyclic)?;
            let original = std::mem::replace(&mut self.objective, replacement);
            if self.output_objective.is_none() {
                self.output_objective = Some(OutputObjective::new(self.sense, original, true));
            }
        }

        let mut constraint_replacements = BTreeMap::new();
        for (&constraint_id, constraint) in self.constraint_collection.active() {
            if affected_constraint_ids.contains(&constraint_id) {
                let mut constraint = constraint.clone();
                substitute_acyclic(&mut constraint.stage.function, acyclic)?;
                constraint_replacements.insert(constraint_id, constraint);
            }
        }
        self.constraint_collection
            .replace_active_rows(constraint_replacements)
            .expect("replacement IDs were read from active constraints");

        for (&cid, ic) in self.indicator_constraint_collection.active().iter() {
            if substituted_variables.contains(&ic.indicator_variable) {
                return Err(SubstitutionError::IndicatorVariableSubstitution {
                    indicator_variable: ic.indicator_variable,
                    constraint_id: cid,
                });
            }
        }
        let mut indicator_replacements = BTreeMap::new();
        for (&constraint_id, ic) in self.indicator_constraint_collection.active() {
            let required_ids = ic.stage.function.required_ids();
            if !required_ids.is_disjoint(&substituted_variables) {
                let mut ic = ic.clone();
                substitute_acyclic(&mut ic.stage.function, acyclic)?;
                indicator_replacements.insert(constraint_id, ic);
            }
        }
        self.indicator_constraint_collection
            .replace_active_rows(indicator_replacements)
            .expect("replacement IDs were read from active indicator constraints");

        for (&cid, oh) in self.one_hot_constraint_collection.active().iter() {
            for var_id in &oh.variables {
                if substituted_variables.contains(var_id) {
                    return Err(SubstitutionError::OneHotVariableSubstitution {
                        variable: *var_id,
                        constraint_id: cid,
                    });
                }
            }
        }
        for (&cid, sos1) in self.sos1_constraint_collection.active().iter() {
            for var_id in &sos1.variables {
                if substituted_variables.contains(var_id) {
                    return Err(SubstitutionError::Sos1VariableSubstitution {
                        variable: *var_id,
                        constraint_id: cid,
                    });
                }
            }
        }

        // Compose reconstruction rules; named functions use the reconstructed state.
        substitute_acyclic(&mut self.decision_variable_dependency, acyclic)?;

        Ok(self)
    }

    fn substitute_one(
        self,
        assigned: VariableID,
        f: &Function,
    ) -> Result<Self::Output, SubstitutionError> {
        substitute_one_via_acyclic(self, assigned, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, constraint::Equality, linear, DecisionVariable, Evaluate, Sense};
    use std::collections::BTreeMap;

    #[test]
    fn repeated_substitution_reports_the_assigned_variable() {
        let id = VariableID::from(0);
        let source = Instance::new(
            Sense::Minimize,
            Function::from(linear!(0)),
            BTreeMap::from([
                (id, DecisionVariable::binary()),
                (VariableID::from(1), DecisionVariable::binary()),
            ]),
            BTreeMap::new(),
        )
        .unwrap();
        let mut instance = source
            .clone()
            .substitute_one(id, &Function::from(linear!(1)))
            .unwrap();
        let before = instance.clone();
        assert_eq!(
            crate::substitute_one(&mut instance, id, &Function::Zero).unwrap_err(),
            SubstitutionError::AssignedVariableSubstitution { variable: id },
        );
        assert_eq!(instance, before);

        let mut parametric = ParametricInstance::from(source)
            .substitute_one(id, &Function::from(linear!(1)))
            .unwrap();
        let before = parametric.clone();
        assert_eq!(
            crate::substitute_one(&mut parametric, id, &Function::Zero).unwrap_err(),
            SubstitutionError::AssignedVariableSubstitution { variable: id },
        );
        assert_eq!(parametric, before);
    }

    #[test]
    fn substitution_preserves_reporting_expressions_and_composes_reconstruction() {
        use crate::{quadratic, Bound, Coefficient, Kind, NamedFunctionID};
        let atol = ATol::default();
        let x = VariableID::from(0);
        let dependent = VariableID::from(1);
        let huge = Coefficient::try_from(f64::MAX).unwrap();
        // Finite at every original integer value, but expansion under
        // x = b0 + b1 - 1 would introduce coefficients of magnitude 2 * MAX.
        let metric = Function::from(((-huge * quadratic!(0, 0)).unwrap() + huge).unwrap());
        let original = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(0)))
            .decision_variables(BTreeMap::from([
                (
                    x,
                    DecisionVariable::new(Kind::Integer, Bound::new(-1.0, 1.0).unwrap(), atol)
                        .unwrap(),
                ),
                (dependent, DecisionVariable::continuous()),
                (VariableID::from(2), DecisionVariable::binary()),
                (VariableID::from(3), DecisionVariable::binary()),
            ]))
            .decision_variable_dependency(crate::assign! { 1 <- linear!(0) + coeff!(1.0) })
            .active_constraints(BTreeMap::from([(
                ConstraintID::from(4),
                Constraint::less_than_or_equal_to_zero(Function::from(linear!(0) + coeff!(-1.0))),
            )]))
            .inactive_constraints(BTreeMap::from([(
                ConstraintID::from(5),
                (
                    Constraint::less_than_or_equal_to_zero(Function::from(
                        linear!(0) + coeff!(-1.0),
                    )),
                    crate::constraint::InactiveConstraintMetadata {
                        skip_feasibility_check: false,
                        reason: "retained source".into(),
                        ..Default::default()
                    },
                ),
            )]))
            .named_functions(BTreeMap::from([
                (NamedFunctionID::from(0), NamedFunction { function: metric }),
                (
                    NamedFunctionID::from(1),
                    NamedFunction {
                        function: Function::from(quadratic!(1, 1)).abs(),
                    },
                ),
            ]))
            .build()
            .unwrap();
        let mut original = original;
        original
            .set_named_function_label(
                NamedFunctionID::from(0),
                ModelingLabel {
                    name: Some("source_metric".into()),
                    subscripts: vec![3],
                    ..Default::default()
                },
            )
            .unwrap();

        for operation in 0..4 {
            let mut encoded = original.clone();
            let assignments = crate::assign! { 0 <- ((linear!(2) + linear!(3)).unwrap() + coeff!(-1.0)).unwrap() };
            match operation {
                0 => {
                    encoded.log_encode([x], atol).unwrap();
                }
                1 => {
                    encoded.unary_encode([x], 2, atol).unwrap();
                }
                2 => {
                    encoded = encoded.substitute_acyclic(&assignments).unwrap();
                }
                _ => {
                    let parametric = ParametricInstance::from(encoded)
                        .substitute_acyclic(&assignments)
                        .unwrap();
                    assert_eq!(
                        parametric.named_function_table(),
                        original.named_function_table()
                    );
                    encoded = parametric
                        .with_parameters(crate::v1::Parameters::default())
                        .unwrap();
                }
            }
            assert_eq!(
                encoded.named_function_table(),
                original.named_function_table()
            );
            assert_eq!(encoded.constraint_context(), original.constraint_context());
            assert_eq!(
                encoded.inactive_constraints(),
                original.inactive_constraints()
            );
            assert_eq!(
                encoded.output_objective().unwrap().function(),
                original.objective()
            );
            assert!(encoded
                .decision_variable_dependency()
                .get(&dependent)
                .is_some());
            let ids: Vec<_> = encoded.active_decision_variable_ids().into_iter().collect();
            assert_eq!(ids.len(), 2);
            for mask in 0..4 {
                let state = crate::v1::State::from_iter(
                    ids.iter()
                        .enumerate()
                        .map(|(i, id)| (id.into_inner(), ((mask >> i) & 1) as f64)),
                );
                let actual = encoded.evaluate(&state, atol).unwrap();
                let restored = actual.state();
                let expected = original
                    .evaluate(
                        &crate::v1::State::from_iter([(0, restored.entries[&0])]),
                        atol,
                    )
                    .unwrap();
                assert_eq!(restored.entries[&1], restored.entries[&0] + 1.0);
                assert_eq!(
                    actual.evaluated_named_function_table(),
                    expected.evaluated_named_function_table()
                );
                assert_eq!(actual.objective(), expected.objective());
            }
        }
    }

    #[test]
    fn test_instance_substitute() {
        // Create decision variables
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(VariableID::from(1), DecisionVariable::continuous());
        decision_variables.insert(VariableID::from(2), DecisionVariable::continuous());
        decision_variables.insert(VariableID::from(3), DecisionVariable::continuous());

        // Create a simple instance: minimize x1 + 2*x2, subject to x1 + x2 <= 10
        let objective = Function::from((linear!(1) + coeff!(2.0) * linear!(2)).unwrap());
        let constraint_function =
            Function::from(((linear!(1) + linear!(2)).unwrap() + coeff!(-10.0)).unwrap());

        let mut constraints = BTreeMap::new();
        let constraint = Constraint {
            equality: Equality::LessThanOrEqualToZero,
            stage: crate::constraint::CreatedData {
                function: constraint_function,
            },
        };
        constraints.insert(ConstraintID::from(1), constraint);

        let mut instance =
            Instance::new(Sense::Minimize, objective, decision_variables, constraints).unwrap();
        let named_function_id = instance
            .new_named_function(
                Function::from((linear!(1) + linear!(2)).unwrap()),
                Some("tracked".to_string()),
                vec![],
                Default::default(),
                None,
            )
            .unwrap();

        // Substitute x1 with x3 + 1
        let substitution = Function::from(linear!(3) + coeff!(1.0));
        let result = instance
            .substitute_one(VariableID::from(1), &substitution)
            .unwrap();

        // Check that the decision_variable_dependency contains the assignment x1 <- x3 + 1
        assert_eq!(result.decision_variable_dependency.len(), 1);
        assert!(result
            .decision_variable_dependency
            .get(&VariableID::from(1))
            .is_some());

        let named_function = result.named_functions().get(&named_function_id).unwrap();
        let expected_ids: std::collections::BTreeSet<_> =
            [VariableID::from(1), VariableID::from(2)]
                .into_iter()
                .collect();
        assert_eq!(named_function.required_ids(), expected_ids);
        let solution = result
            .evaluate(
                &crate::v1::State::from_iter([(2, 2.0), (3, 3.0)]),
                ATol::default(),
            )
            .unwrap();
        assert_eq!(solution.state().entries[&1], 4.0);
        assert_eq!(
            solution.evaluated_named_functions()[&named_function_id].evaluated_value(),
            6.0
        );
    }

    #[test]
    fn fresh_variable_substitution_rejects_unowned_rhs_atomically() {
        let id = VariableID::from(0);
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(0)))
            .decision_variables(BTreeMap::from([(id, DecisionVariable::continuous())]))
            .active_constraints(BTreeMap::new())
            .build()
            .unwrap();
        let acyclic = crate::AcyclicAssignments::new([(id, Function::from(linear!(999)))]).unwrap();
        let before = instance.clone();

        let err = instance
            .apply_encoding_substitution(&acyclic, Vec::new(), ATol::default())
            .unwrap_err();

        assert!(err.to_string().contains("unowned decision variable"));
        assert_eq!(instance, before);
    }

    #[test]
    fn test_parametric_instance_substitute_parameterized_rhs() {
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(VariableID::from(0), DecisionVariable::continuous());
        decision_variables.insert(VariableID::from(1), DecisionVariable::continuous());

        let parameters = ParameterTable::from_ids([VariableID::from(100)].into_iter().collect());

        let objective = Function::from(linear!(0) + linear!(100));
        let parametric = ParametricInstance::new(
            Sense::Minimize,
            objective,
            decision_variables,
            parameters,
            BTreeMap::new(),
        )
        .unwrap();

        let substituted = parametric
            .substitute_one(
                VariableID::from(0),
                &Function::from(linear!(1) + linear!(100)),
            )
            .unwrap();

        assert_eq!(substituted.decision_variable_dependency.len(), 1);
        assert!(substituted
            .decision_variable_dependency
            .get(&VariableID::from(0))
            .is_some());

        let mut parameter_values = crate::v1::Parameters::default();
        parameter_values.entries.insert(100, 2.0);
        let instance = substituted.with_parameters(parameter_values).unwrap();
        let state = crate::v1::State::from_iter([(1, 3.0)]);
        let value = instance
            .objective()
            .evaluate(&state, crate::ATol::default())
            .unwrap();
        assert_eq!(value, 7.0);
        let solution = instance.evaluate(&state, crate::ATol::default()).unwrap();
        assert_eq!(*solution.objective(), 7.0);
    }

    #[test]
    fn parametric_substitution_preserves_existing_output_objective() {
        let decision_variables = BTreeMap::from([
            (VariableID::from(0), DecisionVariable::continuous()),
            (VariableID::from(1), DecisionVariable::continuous()),
        ]);
        let constraint = Constraint::equal_to_zero(Function::from(linear!(0) + coeff!(-1.0)));
        let mut instance = Instance::new(
            Sense::Minimize,
            Function::from(linear!(0)),
            decision_variables,
            BTreeMap::from([(ConstraintID::from(0), constraint)]),
        )
        .unwrap();
        assert!(instance.convert_active_objective(Sense::Maximize));
        let parametric = instance.penalty_method().unwrap();
        let output = parametric.output_objective().cloned().unwrap();

        let substituted = parametric
            .substitute_one(VariableID::from(0), &Function::from(linear!(1)))
            .unwrap();

        assert_eq!(substituted.output_objective(), Some(&output));
        let parameters = crate::v1::Parameters {
            entries: substituted
                .parameters()
                .keys()
                .map(|id| (id.into_inner(), 2.0))
                .collect(),
        };
        let materialized = substituted.with_parameters(parameters).unwrap();
        assert_eq!(materialized.output_objective(), Some(&output));
        let solution = materialized
            .evaluate(&crate::v1::State::from_iter([(1, 1.0)]), ATol::default())
            .unwrap();
        assert_eq!(*solution.objective(), 1.0);
    }

    #[test]
    fn test_parametric_instance_substitute_parameter_target_fails() {
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(VariableID::from(0), DecisionVariable::continuous());

        let parameters = ParameterTable::from_ids([VariableID::from(100)].into_iter().collect());

        let parametric = ParametricInstance::new(
            Sense::Minimize,
            Function::from(linear!(0) + linear!(100)),
            decision_variables,
            parameters,
            BTreeMap::new(),
        )
        .unwrap();

        let err = parametric
            .substitute_one(VariableID::from(100), &Function::from(linear!(0)))
            .unwrap_err();
        assert!(matches!(
            err,
            SubstitutionError::ParameterSubstitution { parameter }
                if parameter == VariableID::from(100)
        ));
    }

    #[test]
    fn test_parametric_instance_substitute_undefined_rhs_fails() {
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(VariableID::from(0), DecisionVariable::continuous());

        let parametric = ParametricInstance::new(
            Sense::Minimize,
            Function::from(linear!(0)),
            decision_variables,
            ParameterTable::default(),
            BTreeMap::new(),
        )
        .unwrap();

        let err = parametric
            .substitute_one(VariableID::from(0), &Function::from(linear!(999)))
            .unwrap_err();
        assert!(matches!(
            err,
            SubstitutionError::UndefinedSubstitutionVariable { variable }
                if variable == VariableID::from(999)
        ));
    }

    #[test]
    fn test_substitute_indicator_function() {
        // Substituting a variable in the indicator's function should work
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(VariableID::from(1), DecisionVariable::continuous());
        decision_variables.insert(VariableID::from(2), DecisionVariable::continuous());
        decision_variables.insert(VariableID::from(10), DecisionVariable::binary());

        let objective = Function::from(linear!(1));

        let mut indicator_constraints = BTreeMap::new();
        indicator_constraints.insert(
            crate::IndicatorConstraintID::from(1),
            crate::IndicatorConstraint::new(
                VariableID::from(10),
                Equality::LessThanOrEqualToZero,
                Function::from(linear!(1) + coeff!(-5.0)),
            ),
        );

        let instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(objective)
            .decision_variables(decision_variables)
            .active_constraints(BTreeMap::new())
            .active_indicator_constraints(indicator_constraints)
            .build()
            .unwrap();

        // Substitute x1 = x2 + 1
        let assignments = crate::AcyclicAssignments::new(vec![(
            VariableID::from(1),
            Function::from(linear!(2) + coeff!(1.0)),
        )])
        .unwrap();

        let result = instance.substitute_acyclic(&assignments).unwrap();

        // Indicator constraint should still exist with substituted function
        assert_eq!(result.active_indicator_constraints().len(), 1);
    }

    #[test]
    fn test_substitute_indicator_variable_fails() {
        // Substituting the indicator variable itself should fail
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(VariableID::from(1), DecisionVariable::continuous());
        decision_variables.insert(VariableID::from(10), DecisionVariable::binary());

        let objective = Function::from(linear!(1));

        let mut indicator_constraints = BTreeMap::new();
        indicator_constraints.insert(
            crate::IndicatorConstraintID::from(1),
            crate::IndicatorConstraint::new(
                VariableID::from(10),
                Equality::LessThanOrEqualToZero,
                Function::from(linear!(1) + coeff!(-5.0)),
            ),
        );

        let instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(objective)
            .decision_variables(decision_variables)
            .active_constraints(BTreeMap::new())
            .active_indicator_constraints(indicator_constraints)
            .build()
            .unwrap();

        // Try to substitute the indicator variable x10
        let assignments = crate::AcyclicAssignments::new(vec![(
            VariableID::from(10),
            Function::from(coeff!(1.0)),
        )])
        .unwrap();

        let result = instance.substitute_acyclic(&assignments);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SubstitutionError::IndicatorVariableSubstitution {
                indicator_variable,
                constraint_id,
            } if indicator_variable == VariableID::from(10)
                && constraint_id == crate::IndicatorConstraintID::from(1)
        ));
    }
}
