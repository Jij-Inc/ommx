use super::*;
use std::collections::BTreeMap;

impl Instance {
    /// Reduce binary powers in the active objective and constraints.
    ///
    /// For binary variables, x^n = x for any n >= 1. Binary variable IDs are
    /// derived from this instance's decision-variable table.
    ///
    /// Rewrites the active objective, active regular and indicator constraint
    /// bodies. Named functions,
    /// decision-variable dependency right-hand sides, removed constraints, and
    /// an existing output objective are left unchanged.
    /// One-hot and SOS1 constraints contain variable sets rather than expression
    /// bodies and do not require this rewrite.
    ///
    /// Returns `true` if any supported expression changed. If coefficient
    /// arithmetic fails in any expression, the entire instance is unchanged.
    ///
    /// # Postconditions
    ///
    /// Reduction preserves modeling labels, constraint contexts, and lifecycle
    /// state. If the active objective changes and no output objective exists,
    /// its original value is retained as the output objective.
    ///
    /// ```
    /// use ommx::{
    ///     quadratic, v1::State, ATol, DecisionVariable, Evaluate, Function, Instance,
    ///     Sense, VariableID,
    /// };
    /// use std::collections::{BTreeMap, HashMap};
    ///
    /// let mut instance = Instance::builder()
    ///     .sense(Sense::Maximize)
    ///     .objective(Function::from(quadratic!(1, 1)))
    ///     .decision_variables(BTreeMap::from([(
    ///         VariableID::from(1),
    ///         DecisionVariable::binary(),
    ///     )]))
    ///     .constraints(BTreeMap::new())
    ///     .build()
    ///     .unwrap();
    /// let original = instance.objective().clone();
    ///
    /// assert!(instance.reduce_binary_power().unwrap());
    /// let output = instance.output_objective().unwrap();
    /// assert_eq!(output.sense(), Sense::Maximize);
    /// assert_eq!(output.function(), &original);
    /// assert!(!instance.reduce_binary_power().unwrap());
    /// let state = State::from(HashMap::from([(1, 1.0)]));
    /// assert_eq!(instance.objective().evaluate(&state, ATol::default()).unwrap(), 1.0);
    /// let solution = instance.evaluate(&state, ATol::default()).unwrap();
    /// assert_eq!(*solution.objective(), 1.0);
    /// ```
    pub fn reduce_binary_power(&mut self) -> Result<bool, crate::CoefficientError> {
        let binary_ids = self.binary_ids();
        if binary_ids.is_empty() {
            return Ok(false);
        }

        // Plan all Active Formulation expression rewrites before mutating `self`. Row
        // replacements leave collection-owned labels and contexts in place.
        let objective_replacement = self.objective.plan_binary_power_reduction(&binary_ids)?;
        let mut replacements = BTreeMap::new();
        for (&id, constraint) in self.constraint_collection.active() {
            if let Some(replacement) = constraint.plan_binary_power_reduction(&binary_ids)? {
                replacements.insert(id, replacement);
            }
        }
        let mut indicator_replacements = BTreeMap::new();
        for (&id, constraint) in self.indicator_constraint_collection.active() {
            if let Some(function) = constraint
                .stage
                .function
                .plan_binary_power_reduction(&binary_ids)?
            {
                indicator_replacements.insert(
                    id,
                    IndicatorConstraint::new(
                        constraint.indicator_variable,
                        constraint.equality,
                        function,
                    ),
                );
            }
        }
        let changed = objective_replacement.is_some()
            || !replacements.is_empty()
            || !indicator_replacements.is_empty();

        // Commit only after every fallible rewrite has succeeded.
        if let Some(replacement) = objective_replacement {
            self.replace_active_objective_preserving_output(replacement);
        }
        if !replacements.is_empty() {
            self.constraint_collection
                .replace_active_rows(replacements)
                .expect("replacement IDs were read from active constraints");
        }
        self.indicator_constraint_collection
            .replace_active_rows(indicator_replacements)
            .expect("replacement IDs were read from active indicator constraints");
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coeff, constraint::CreatedData, linear, quadratic, Bound, Coefficient, Constraint,
        DecisionVariable, Equality, Kind, Sense,
    };
    use ::approx::assert_abs_diff_eq;
    use proptest::prelude::*;

    #[test]
    fn test_instance_reduce_binary_power() {
        // Create instance with binary and continuous variables
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(VariableID::from(1), DecisionVariable::binary());
        decision_variables.insert(
            VariableID::from(2),
            DecisionVariable::new(
                Kind::Continuous,
                Bound::new(0.0, 10.0).unwrap(),
                crate::ATol::default(),
            )
            .unwrap(),
        );

        // Objective: x1^2 + x1*x2 + x2^2
        let mut objective_poly =
            (quadratic!(1, 1) + (coeff!(2.0) * quadratic!(1, 2)).unwrap()).unwrap();
        objective_poly = (objective_poly + (coeff!(3.0) * quadratic!(2, 2)).unwrap()).unwrap();
        let objective = Function::Quadratic(objective_poly);

        // Constraint: x1^2 + x2 <= 5  (i.e., x1^2 + x2 - 5 <= 0)
        let mut constraints = BTreeMap::new();
        let constraint_func = Function::Quadratic(
            ((quadratic!(1, 1) + quadratic!(2)).unwrap() + coeff!(-5.0)).unwrap(),
        );
        constraints.insert(
            ConstraintID::from(1),
            Constraint {
                equality: Equality::LessThanOrEqualToZero,
                stage: CreatedData {
                    function: constraint_func,
                },
            },
        );

        let mut instance = Instance::new(
            Sense::Minimize,
            objective.clone(),
            decision_variables,
            constraints,
        )
        .unwrap();

        // Apply reduction
        let changed = instance.reduce_binary_power().unwrap();
        assert!(changed);
        assert_eq!(instance.output_objective().unwrap().function(), &objective);

        // Check objective: x1^2 -> x1
        let mut expected_objective_poly =
            (quadratic!(1) + (coeff!(2.0) * quadratic!(1, 2)).unwrap()).unwrap();
        expected_objective_poly =
            (expected_objective_poly + (coeff!(3.0) * quadratic!(2, 2)).unwrap()).unwrap();
        let expected_objective = Function::Quadratic(expected_objective_poly);
        assert_abs_diff_eq!(instance.objective(), &expected_objective);

        // Check constraint: x1^2 -> x1
        let expected_constraint_func =
            Function::Quadratic(((quadratic!(1) + quadratic!(2)).unwrap() + coeff!(-5.0)).unwrap());
        assert_eq!(
            instance
                .constraints()
                .get(&ConstraintID::from(1))
                .unwrap()
                .function(),
            &expected_constraint_func
        );

        // Apply reduction again - should not change
        let changed2 = instance.reduce_binary_power().unwrap();
        assert!(!changed2);
    }

    #[test]
    fn test_instance_reduce_binary_power_no_binary() {
        // Create instance with only continuous variables
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(VariableID::from(1), DecisionVariable::continuous());
        decision_variables.insert(VariableID::from(2), DecisionVariable::integer());

        let objective = Function::Quadratic(
            (quadratic!(1, 1) + (coeff!(2.0) * quadratic!(2, 2)).unwrap()).unwrap(),
        );

        let mut instance = Instance::new(
            Sense::Minimize,
            objective.clone(),
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();

        // Apply reduction - should not change since no binary variables
        let changed = instance.reduce_binary_power().unwrap();
        assert!(!changed);
        assert_eq!(instance.objective(), &objective);
        assert!(instance.output_objective().is_none());
    }

    #[test]
    fn reduce_binary_power_does_not_capture_constraint_only_rewrite() {
        let variable = VariableID::from(1);
        let objective = Function::from(linear!(1));
        let constraint = Constraint::equal_to_zero(Function::from(quadratic!(1, 1)));
        let mut instance = Instance::new(
            Sense::Minimize,
            objective.clone(),
            BTreeMap::from([(variable, DecisionVariable::binary())]),
            BTreeMap::from([(ConstraintID::from(1), constraint)]),
        )
        .unwrap();

        assert!(instance.reduce_binary_power().unwrap());

        assert_eq!(instance.objective(), &objective);
        assert!(instance.output_objective().is_none());
    }

    #[test]
    fn reduce_binary_power_moves_original_expression_to_output_objective() {
        let variable = VariableID::from(1);
        let objective = Function::from(linear!(1)).powi(2);
        let mut instance = Instance::new(
            Sense::Minimize,
            objective,
            BTreeMap::from([(variable, DecisionVariable::binary())]),
            BTreeMap::new(),
        )
        .unwrap();
        let Function::Expression(expression) = instance.objective() else {
            panic!("powi(2) must create an expression")
        };
        let original_instructions = crate::function::operation::instructions(expression).as_ptr();

        assert!(instance.reduce_binary_power().unwrap());

        let Function::Expression(expression) = instance.output_objective().unwrap().function()
        else {
            panic!("output objective must preserve the original expression")
        };
        assert_eq!(
            crate::function::operation::instructions(expression).as_ptr(),
            original_instructions
        );
        assert_eq!(instance.objective(), &Function::from(linear!(1)));
    }

    #[test]
    fn reduce_binary_power_preserves_instance_on_coefficient_error() {
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(VariableID::from(1), DecisionVariable::binary());

        let objective = Function::Quadratic(quadratic!(1, 1).into());
        let huge = Coefficient::try_from(f64::MAX).unwrap();
        let overflowing_constraint = Function::Quadratic(
            ((huge * quadratic!(1, 1)).unwrap() + (huge * quadratic!(1)).unwrap()).unwrap(),
        );
        let mut constraints = BTreeMap::new();
        constraints.insert(
            ConstraintID::from(1),
            Constraint {
                equality: Equality::LessThanOrEqualToZero,
                stage: CreatedData {
                    function: overflowing_constraint,
                },
            },
        );
        let mut instance =
            Instance::new(Sense::Minimize, objective, decision_variables, constraints).unwrap();
        let before_objective = instance.objective().clone();
        let before_constraints = instance.constraints().clone();

        let err = instance.reduce_binary_power().unwrap_err();

        assert_eq!(err, crate::CoefficientError::Infinite);
        assert_eq!(instance.objective(), &before_objective);
        assert_eq!(instance.constraints(), &before_constraints);
        assert!(instance.output_objective().is_none());
    }

    // The first three slots are active expressions; the last two
    // are retained for output evaluation and variable reconstruction.
    fn instance_with_expression_owners(functions: [Function; 5]) -> Instance {
        let [objective, regular, indicator, named, dependency] = functions;
        let reason = crate::constraint::RemovedReason {
            reason: "retained for diagnostics".into(),
            ..Default::default()
        };
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(objective)
            .decision_variables(BTreeMap::from([
                (VariableID::from(1), DecisionVariable::binary()),
                (VariableID::from(2), DecisionVariable::continuous()),
                (VariableID::from(3), DecisionVariable::continuous()),
            ]))
            .constraints(BTreeMap::from([(
                ConstraintID::from(10),
                Constraint::equal_to_zero(regular),
            )]))
            .indicator_constraints(BTreeMap::from([(
                crate::IndicatorConstraintID::from(11),
                crate::IndicatorConstraint::new(
                    VariableID::from(1),
                    Equality::LessThanOrEqualToZero,
                    indicator,
                ),
            )]))
            .named_functions(BTreeMap::from([(
                NamedFunctionID::from(12),
                NamedFunction { function: named },
            )]))
            .decision_variable_dependency(
                AcyclicAssignments::new([(VariableID::from(3), dependency)]).unwrap(),
            )
            .removed_constraints(BTreeMap::from([(
                ConstraintID::from(13),
                (
                    Constraint::equal_to_zero(Function::from(quadratic!(1, 1))),
                    reason.clone(),
                ),
            )]))
            .removed_indicator_constraints(BTreeMap::from([(
                crate::IndicatorConstraintID::from(14),
                (
                    crate::IndicatorConstraint::new(
                        VariableID::from(1),
                        Equality::EqualToZero,
                        Function::from(quadratic!(1, 1)),
                    ),
                    reason,
                ),
            )]))
            .build()
            .unwrap();
        let label = crate::ModelingLabel {
            name: Some("retained label".into()),
            subscripts: vec![4],
            ..Default::default()
        };
        let context = crate::ConstraintContext {
            label: label.clone(),
            provenance: vec![crate::constraint::Provenance::IndicatorConstraint(
                crate::IndicatorConstraintID::from(14),
            )],
        };
        instance
            .set_constraint_context(ConstraintID::from(10), context.clone())
            .unwrap();
        instance
            .set_indicator_constraint_context(crate::IndicatorConstraintID::from(11), context)
            .unwrap();
        instance
            .set_named_function_label(NamedFunctionID::from(12), label)
            .unwrap();
        instance
    }

    fn owned_functions(instance: &Instance) -> [&Function; 5] {
        [
            instance.objective(),
            instance.constraints()[&ConstraintID::from(10)].function(),
            &instance.indicator_constraints()[&crate::IndicatorConstraintID::from(11)]
                .stage
                .function,
            &instance.named_functions()[&NamedFunctionID::from(12)].function,
            instance
                .decision_variable_dependency()
                .get(&VariableID::from(3))
                .unwrap(),
        ]
    }

    #[test]
    fn reduce_binary_power_covers_each_active_expression_owner() {
        // A continuous square must survive beside the reducible binary square.
        let reducible = Function::from((quadratic!(1, 1) + quadratic!(2, 2)).unwrap());
        let reduced = Function::from((quadratic!(1) + quadratic!(2, 2)).unwrap());
        for owner in 0..3 {
            let mut functions = std::array::from_fn(|_| Function::from(linear!(1)));
            functions[owner] = reducible.clone();
            functions[3] = reducible.clone();
            functions[4] = reducible.clone();
            let mut instance = instance_with_expression_owners(functions);
            let before = instance.clone();

            assert!(instance.reduce_binary_power().unwrap(), "owner {owner}");
            assert_eq!(owned_functions(&instance)[owner], &reduced, "owner {owner}");
            for other in (0..5).filter(|&other| other != owner) {
                assert_eq!(
                    owned_functions(&instance)[other],
                    owned_functions(&before)[other]
                );
            }
            assert_eq!(instance.constraint_context(), before.constraint_context());
            assert_eq!(
                instance.indicator_constraint_context(),
                before.indicator_constraint_context()
            );
            assert_eq!(
                instance.named_function_labels(),
                before.named_function_labels()
            );
            assert_eq!(instance.removed_constraints(), before.removed_constraints());
            assert_eq!(
                instance.removed_indicator_constraints(),
                before.removed_indicator_constraints()
            );
            assert_eq!(instance.output_objective().is_some(), owner == 0);
            assert!(!instance.reduce_binary_power().unwrap());
        }
    }

    #[test]
    fn reduce_binary_power_rewrites_composed_functions_together() {
        let original = Function::from(linear!(1)).powi(3);
        let mut instance =
            instance_with_expression_owners(std::array::from_fn(|_| original.clone()));

        assert!(instance.reduce_binary_power().unwrap());
        for function in &owned_functions(&instance)[..3] {
            assert_eq!(*function, &Function::from(linear!(1)));
            for value in [0.0, 1.0] {
                let state = crate::v1::State::from(std::collections::HashMap::from([(1, value)]));
                assert_eq!(
                    function.evaluate(&state, crate::ATol::default()).unwrap(),
                    original.evaluate(&state, crate::ATol::default()).unwrap(),
                );
            }
        }
        for function in &owned_functions(&instance)[3..] {
            assert_eq!(*function, &original);
        }
        assert_eq!(instance.output_objective().unwrap().function(), &original);
        assert!(!instance.reduce_binary_power().unwrap());
    }

    #[test]
    fn reduce_binary_power_preserves_reducible_dependency_and_its_graph() {
        let mut functions = std::array::from_fn(|_| Function::from(linear!(1)));
        functions[4] =
            Function::from(((quadratic!(1, 1) - quadratic!(1)).unwrap() + quadratic!(2)).unwrap());
        let mut instance = instance_with_expression_owners(functions);
        let before = instance.clone();

        assert!(!instance.reduce_binary_power().unwrap());
        // Instance equality includes the assignments and their cached graph.
        assert_eq!(instance, before);
    }

    #[test]
    fn reduce_binary_power_rolls_back_on_each_late_owner_error() {
        let huge = Coefficient::try_from(f64::MAX).unwrap();
        let overflowing = Function::from(
            ((huge * quadratic!(1, 1)).unwrap() + (huge * quadratic!(1)).unwrap()).unwrap(),
        );
        for owner in 1..3 {
            let mut functions = std::array::from_fn(|_| Function::from(quadratic!(1, 1)));
            functions[owner] = overflowing.clone();
            let mut instance = instance_with_expression_owners(functions);
            let before = instance.clone();

            assert_eq!(
                instance.reduce_binary_power(),
                Err(crate::CoefficientError::Infinite),
                "owner {owner}"
            );
            assert_eq!(instance, before, "owner {owner}");
        }
    }

    #[test]
    fn reduce_binary_power_preserves_overflowing_output_and_dependency_expressions() {
        let huge = Coefficient::try_from(f64::MAX).unwrap();
        let overflowing = Function::from(
            ((huge * quadratic!(1, 1)).unwrap() + (huge * quadratic!(1)).unwrap()).unwrap(),
        );
        for active_changes in [false, true] {
            let active = if active_changes {
                Function::from(quadratic!(1, 1))
            } else {
                Function::from(linear!(1))
            };
            let mut instance = instance_with_expression_owners([
                active.clone(),
                active.clone(),
                active,
                overflowing.clone(),
                overflowing.clone(),
            ]);
            let before = instance.clone();

            assert_eq!(instance.reduce_binary_power().unwrap(), active_changes);
            assert_eq!(
                instance.named_function_table(),
                before.named_function_table()
            );
            assert_eq!(
                instance.decision_variable_dependency(),
                before.decision_variable_dependency()
            );
            assert_eq!(instance.removed_constraints(), before.removed_constraints());
            assert_eq!(
                instance.removed_indicator_constraints(),
                before.removed_indicator_constraints()
            );
            if !active_changes {
                assert_eq!(instance, before);
            }
        }
    }

    proptest! {
        #[test]
        fn test_instance_reduce_binary_power_idempotent(
            mut instance in Instance::arbitrary()
        ) {
            let before = instance.clone();
            let _first = instance.reduce_binary_power().unwrap();
            prop_assert_eq!(instance.named_function_table(), before.named_function_table());
            prop_assert_eq!(instance.decision_variable_dependency(), before.decision_variable_dependency());
            prop_assert_eq!(instance.removed_constraints(), before.removed_constraints());
            prop_assert_eq!(instance.removed_indicator_constraints(), before.removed_indicator_constraints());
            prop_assert_eq!(instance.one_hot_constraint_collection(), before.one_hot_constraint_collection());
            prop_assert_eq!(instance.sos1_constraint_collection(), before.sos1_constraint_collection());
            if before.output_objective().is_some() {
                prop_assert_eq!(instance.output_objective(), before.output_objective());
            }
            let second = instance.reduce_binary_power().unwrap();
            prop_assert!(!second, "reduce_binary_power should be idempotent");
        }
    }
}
