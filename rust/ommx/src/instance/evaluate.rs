use super::*;
use crate::{
    constraint::RemovedReason, indicator_constraint::IndicatorPropagateOutput, ATol, Evaluate,
    Propagate, VariableIDSet,
};
use anyhow::{anyhow, Result};
use std::collections::BTreeMap;

impl Evaluate for Instance {
    type Output = crate::Solution;
    type SampledOutput = crate::SampleSet;

    fn evaluate(&self, state: &v1::State, atol: ATol) -> Result<Self::Output> {
        let state = self
            .analyze_decision_variables()
            .populate(state.clone(), atol)?;

        let objective = self.objective.evaluate(&state, atol)?;
        let evaluated_constraints = self.constraint_collection.evaluate(&state, atol)?;
        let evaluated_indicator_constraints = self
            .indicator_constraint_collection
            .evaluate(&state, atol)?;
        let evaluated_one_hot_constraints =
            self.one_hot_constraint_collection.evaluate(&state, atol)?;
        let evaluated_sos1_constraints = self.sos1_constraint_collection.evaluate(&state, atol)?;

        let mut decision_variables = BTreeMap::default();
        for dv in self.decision_variables.values() {
            let evaluated_dv = dv.evaluate(&state, atol)?;
            decision_variables.insert(*evaluated_dv.id(), evaluated_dv);
        }

        let mut evaluated_named_functions = BTreeMap::default();
        for (id, named_function) in self.named_functions.iter() {
            let evaluated_named_function = named_function.evaluate(&state, atol)?;
            evaluated_named_functions.insert(*id, evaluated_named_function);
        }

        let sense = self.sense();

        // SAFETY: Instance invariants guarantee Solution invariants
        let solution = unsafe {
            crate::Solution::builder()
                .objective(objective)
                .evaluated_constraints_collection(evaluated_constraints)
                .evaluated_indicator_constraints_collection(evaluated_indicator_constraints)
                .evaluated_one_hot_constraints_collection(evaluated_one_hot_constraints)
                .evaluated_sos1_constraints_collection(evaluated_sos1_constraints)
                .evaluated_named_functions(evaluated_named_functions)
                .decision_variables(decision_variables)
                .sense(sense)
                .build_unchecked()?
        };

        Ok(solution)
    }

    fn evaluate_samples(&self, samples: &v1::Samples, atol: ATol) -> Result<Self::SampledOutput> {
        // Populate the decision variables in the samples
        let samples = {
            let analysis = self.analyze_decision_variables();
            let mut samples = samples.clone();
            for sample in samples.states_mut() {
                let sample = sample?;
                let state = std::mem::take(sample);
                let state = analysis.populate(state, atol)?;
                *sample = state;
            }
            samples
        };

        let sampled_constraints: crate::constraint_type::SampledCollection<crate::Constraint> =
            self.constraint_collection
                .evaluate_samples(&samples, atol)?;
        let sampled_indicator_constraints: crate::constraint_type::SampledCollection<
            crate::IndicatorConstraint,
        > = self
            .indicator_constraint_collection
            .evaluate_samples(&samples, atol)?;
        let sampled_one_hot_constraints: crate::constraint_type::SampledCollection<
            crate::OneHotConstraint,
        > = self
            .one_hot_constraint_collection
            .evaluate_samples(&samples, atol)?;
        let sampled_sos1_constraints: crate::constraint_type::SampledCollection<
            crate::Sos1Constraint,
        > = self
            .sos1_constraint_collection
            .evaluate_samples(&samples, atol)?;

        // Objective
        let objectives = self.objective().evaluate_samples(&samples, atol)?;

        // Reconstruct decision variable values
        let mut decision_variables = std::collections::BTreeMap::new();
        for dv in self.decision_variables.values() {
            let sampled_dv = dv.evaluate_samples(&samples, atol)?;
            decision_variables.insert(dv.id(), sampled_dv);
        }

        // Reconstruct named function values
        let mut named_functions = std::collections::BTreeMap::new();
        for (id, named_function) in self.named_functions.iter() {
            let sampled_named_function = named_function.evaluate_samples(&samples, atol)?;
            named_functions.insert(*id, sampled_named_function);
        }

        Ok(crate::SampleSet::builder()
            .decision_variables(decision_variables)
            .objectives(objectives.try_into()?)
            .constraints(sampled_constraints.into_inner())
            .indicator_constraints(sampled_indicator_constraints.into_inner())
            .one_hot_constraints(sampled_one_hot_constraints.into_inner())
            .sos1_constraints(sampled_sos1_constraints.into_inner())
            .named_functions(named_functions)
            .sense(self.sense)
            .build()?)
    }

    fn partial_evaluate(&mut self, state: &v1::State, atol: ATol) -> Result<()> {
        // Phase 1: Propagate through special constraints (unit propagation).
        // This may discover additional variable fixings and consume/transform constraints.
        let expanded_state = self.propagate_special_constraints(state, atol)?;

        // Phase 2: Substitute fixed values into decision variables.
        for (id, value) in expanded_state.entries.iter() {
            let Some(dv) = self.decision_variables.get_mut(&VariableID::from(*id)) else {
                return Err(anyhow!("Unknown decision variable (ID={id}) in state."));
            };
            dv.substitute(*value, atol)?;
        }

        // Phase 3: Regular partial evaluation with expanded state.
        // Special constraint collections are already handled by propagation — not called again.
        self.objective.partial_evaluate(&expanded_state, atol)?;
        self.constraint_collection
            .partial_evaluate(&expanded_state, atol)?;
        for named_function in self.named_functions.values_mut() {
            named_function.partial_evaluate(&expanded_state, atol)?;
        }
        self.decision_variable_dependency
            .partial_evaluate(&expanded_state, atol)?;
        Ok(())
    }

    fn required_ids(&self) -> VariableIDSet {
        self.analyze_decision_variables().used().clone()
    }
}

impl Instance {
    /// Run unit propagation over special constraint types (OneHot, SOS1, Indicator).
    ///
    /// This is a fixed-point iteration: each constraint is propagated with the current
    /// state, and any additional variable fixings are merged back into the state.
    /// The loop continues until no new fixings are discovered.
    ///
    /// Consumed constraints are moved to the removed set.
    /// Promoted indicator constraints are inserted into the regular constraint collection.
    fn propagate_special_constraints(
        &mut self,
        state: &v1::State,
        atol: ATol,
    ) -> Result<v1::State> {
        let mut expanded = state.clone();
        let mut changed = true;

        let propagation_reason = RemovedReason {
            reason: "unit_propagation".to_string(),
            parameters: Default::default(),
        };

        while changed {
            changed = false;

            // --- OneHot constraints ---
            let one_hots = std::mem::take(self.one_hot_constraint_collection.active_mut());
            for (id, mut oh) in one_hots {
                let (transformed, additional) = oh.propagate(&expanded, atol)?;
                if !additional.entries.is_empty() {
                    for (var_id, value) in additional.entries {
                        expanded.entries.insert(var_id, value);
                    }
                    changed = true;
                }
                match transformed {
                    None => {
                        // In-place modification — keep active
                        self.one_hot_constraint_collection
                            .active_mut()
                            .insert(id, oh);
                    }
                    Some(()) => {
                        // Transformed (consumed) — move original to removed
                        self.one_hot_constraint_collection
                            .removed_mut()
                            .insert(id, (oh, propagation_reason.clone()));
                    }
                }
            }

            // --- SOS1 constraints ---
            let sos1s = std::mem::take(self.sos1_constraint_collection.active_mut());
            for (id, mut sos1) in sos1s {
                let (transformed, additional) = sos1.propagate(&expanded, atol)?;
                if !additional.entries.is_empty() {
                    for (var_id, value) in additional.entries {
                        expanded.entries.insert(var_id, value);
                    }
                    changed = true;
                }
                match transformed {
                    None => {
                        self.sos1_constraint_collection
                            .active_mut()
                            .insert(id, sos1);
                    }
                    Some(()) => {
                        self.sos1_constraint_collection
                            .removed_mut()
                            .insert(id, (sos1, propagation_reason.clone()));
                    }
                }
            }

            // --- Indicator constraints ---
            let indicators = std::mem::take(self.indicator_constraint_collection.active_mut());
            for (id, mut ic) in indicators {
                let (transformed, additional) = ic.propagate(&expanded, atol)?;
                if !additional.entries.is_empty() {
                    for (var_id, value) in additional.entries {
                        expanded.entries.insert(var_id, value);
                    }
                    changed = true;
                }
                match transformed {
                    None => {
                        // In-place — keep active
                        self.indicator_constraint_collection
                            .active_mut()
                            .insert(id, ic);
                    }
                    Some(IndicatorPropagateOutput::Promote(constraint)) => {
                        // Validate no ConstraintID collision
                        let cid = constraint.id;
                        if self.constraint_collection.active().contains_key(&cid)
                            || self.constraint_collection.removed().contains_key(&cid)
                        {
                            anyhow::bail!(
                                "Cannot promote indicator constraint {:?}: \
                                 ConstraintID {:?} already exists in constraint collection",
                                id,
                                cid
                            );
                        }
                        self.constraint_collection
                            .active_mut()
                            .insert(cid, constraint);
                        // Move original indicator to removed (preserves full data)
                        self.indicator_constraint_collection
                            .removed_mut()
                            .insert(id, (ic, propagation_reason.clone()));
                    }
                    Some(IndicatorPropagateOutput::Removed) => {
                        // Move original indicator to removed
                        self.indicator_constraint_collection
                            .removed_mut()
                            .insert(id, (ic, propagation_reason.clone()));
                    }
                }
            }
        }

        Ok(expanded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::random::arbitrary_split_state;
    use crate::{coeff, linear};
    use ::approx::AbsDiffEq;
    use proptest::prelude::*;
    use std::collections::HashMap;

    proptest! {
        #[test]
        fn test_evaluate_instance(
            (instance, state) in Instance::arbitrary()
                .prop_flat_map(|instance| {
                    let state = instance.arbitrary_state();
                    (Just(instance), state)
                })
        ) {
            let analysis = instance.analyze_decision_variables();
            let solution = instance.evaluate(&state, ATol::default()).unwrap();
            // Must be populated
            let ids: VariableIDSet = solution.state().entries.keys().map(|id| VariableID::from(*id)).collect();
            prop_assert_eq!(&ids, analysis.all());
        }

        #[test]
        fn partial_evaluate(
            (mut instance, state, (u, v)) in Instance::arbitrary()
                .prop_flat_map(|instance| {
                    let state = instance.arbitrary_state();
                    (Just(instance), state).prop_flat_map(|(instance, state)| {
                        let split = arbitrary_split_state(&state);
                        (Just(instance), Just(state), split)
                    })
                })
        ) {
            let s1 = instance.evaluate(&state, ATol::default()).unwrap();
            instance.partial_evaluate(&u, ATol::default()).unwrap();
            let s2 = instance.evaluate(&v, ATol::default()).unwrap();
            prop_assert!(s1.state().abs_diff_eq(&s2.state(), ATol::default()));
        }
    }

    /// Test that named functions can reference fixed, dependent, and irrelevant variables
    #[test]
    fn test_evaluate_named_function_with_fixed_dependent_irrelevant_variables() {
        use crate::{DecisionVariable, NamedFunction, NamedFunctionID};
        use maplit::btreemap;

        // Create decision variables:
        // x1 (id=1): used in objective
        // x2 (id=2): fixed variable (substituted_value = 3.0)
        // x3 (id=3): dependent on x4 (x3 = 2 * x4)
        // x4 (id=4): irrelevant (not used in objective/constraints)
        // x5 (id=5): only used in named_functions

        let x1 = DecisionVariable::continuous(VariableID::from(1));
        let mut x2 = DecisionVariable::continuous(VariableID::from(2));
        x2.substitute(3.0, ATol::default()).unwrap(); // fixed to 3.0
        let x3 = DecisionVariable::continuous(VariableID::from(3));
        let x4 = DecisionVariable::continuous(VariableID::from(4));
        let x5 = DecisionVariable::continuous(VariableID::from(5));

        let decision_variables = btreemap! {
            VariableID::from(1) => x1,
            VariableID::from(2) => x2,
            VariableID::from(3) => x3,
            VariableID::from(4) => x4,
            VariableID::from(5) => x5,
        };

        // Objective: minimize x1 (only x1 is "used")
        let objective = Function::from(linear!(1));

        // Create instance with dependency: x3 = 2 * x4
        let decision_variable_dependency = crate::AcyclicAssignments::new(vec![(
            VariableID::from(3),
            Function::from(coeff!(2.0) * linear!(4)),
        )])
        .unwrap();

        // Named function: f = x2 + x3 + x4 + x5
        // This references:
        // - x2: fixed variable
        // - x3: dependent variable
        // - x4: irrelevant variable
        // - x5: only used in named function
        let named_function = NamedFunction {
            id: NamedFunctionID::from(1),
            function: Function::from(linear!(2) + linear!(3) + linear!(4) + linear!(5)),
            name: Some("f".to_string()),
            subscripts: vec![],
            parameters: Default::default(),
            description: None,
        };

        let named_functions = btreemap! {
            NamedFunctionID::from(1) => named_function,
        };

        let mut instance = Instance::new(
            Sense::Minimize,
            objective,
            decision_variables,
            BTreeMap::new(), // No constraints
        )
        .unwrap();
        instance.decision_variable_dependency = decision_variable_dependency;
        instance.named_functions = named_functions;

        // Verify the analysis: x1 is used, x2 is fixed, x3 is dependent,
        // x4 and x5 should be irrelevant (named_functions don't contribute to "used")
        let analysis = instance.analyze_decision_variables();
        assert!(analysis.used().contains(&VariableID::from(1)));
        assert!(analysis.fixed().contains_key(&VariableID::from(2)));
        assert!(analysis.dependent().contains_key(&VariableID::from(3)));
        assert!(analysis.irrelevant().contains_key(&VariableID::from(4)));
        assert!(analysis.irrelevant().contains_key(&VariableID::from(5)));

        // Create state: x1=1.0, x4=2.0, x5=10.0
        // x2 is fixed to 3.0
        // x3 is dependent: x3 = 2 * x4 = 4.0
        let state = v1::State::from(HashMap::from([(1, 1.0), (4, 2.0), (5, 10.0)]));

        let solution = instance.evaluate(&state, ATol::default()).unwrap();

        // Check objective value
        assert_eq!(*solution.objective(), 1.0);

        // Check named function evaluation
        // f = x2 + x3 + x4 + x5 = 3.0 + 4.0 + 2.0 + 10.0 = 19.0
        let evaluated_nf = solution
            .evaluated_named_functions()
            .get(&NamedFunctionID::from(1))
            .unwrap();
        assert_eq!(evaluated_nf.evaluated_value(), 19.0);

        // Check used_decision_variable_ids of the evaluated named function
        // It contains the variable IDs referenced in the named function's expression,
        // which are x2, x3, x4, x5 (dependency substitution is done at the state level,
        // not at the expression level)
        let used_ids = evaluated_nf.used_decision_variable_ids();
        assert!(used_ids.contains(&VariableID::from(2)));
        assert!(used_ids.contains(&VariableID::from(3)));
        assert!(used_ids.contains(&VariableID::from(4)));
        assert!(used_ids.contains(&VariableID::from(5)));
        // x1 is not used in the named function
        assert!(!used_ids.contains(&VariableID::from(1)));
    }

    // === Unit propagation integration tests ===

    #[test]
    fn test_partial_evaluate_one_hot_propagation() {
        use crate::{DecisionVariable, OneHotConstraint, OneHotConstraintID};
        use maplit::btreemap;

        // Binary variables x1, x2, x3 with OneHot{x1, x2, x3}
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
            VariableID::from(3) => DecisionVariable::binary(VariableID::from(3)),
        };
        let objective = Function::from(linear!(1) + linear!(2) + linear!(3));

        let mut instance = Instance::new(
            Sense::Minimize,
            objective,
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();

        let oh = OneHotConstraint::new(
            OneHotConstraintID::from(1),
            [1, 2, 3].into_iter().map(VariableID::from).collect(),
        );
        instance
            .one_hot_constraint_collection
            .active_mut()
            .insert(OneHotConstraintID::from(1), oh);

        // Fix x2 = 1 → OneHot propagation should fix x1=0, x3=0
        let state = v1::State::from(HashMap::from([(2, 1.0)]));
        instance.partial_evaluate(&state, ATol::default()).unwrap();

        // All three variables should be substituted
        assert_eq!(
            instance.decision_variables[&VariableID::from(1)].substituted_value(),
            Some(0.0)
        );
        assert_eq!(
            instance.decision_variables[&VariableID::from(2)].substituted_value(),
            Some(1.0)
        );
        assert_eq!(
            instance.decision_variables[&VariableID::from(3)].substituted_value(),
            Some(0.0)
        );

        // OneHot constraint should be consumed (moved to removed)
        assert!(instance.one_hot_constraint_collection.active().is_empty());
        assert_eq!(instance.one_hot_constraint_collection.removed().len(), 1);
    }

    #[test]
    fn test_partial_evaluate_one_hot_unit_propagation() {
        use crate::{DecisionVariable, OneHotConstraint, OneHotConstraintID};
        use maplit::btreemap;

        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
            VariableID::from(3) => DecisionVariable::binary(VariableID::from(3)),
        };
        let objective = Function::from(linear!(1) + linear!(2) + linear!(3));

        let mut instance = Instance::new(
            Sense::Minimize,
            objective,
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();

        let oh = OneHotConstraint::new(
            OneHotConstraintID::from(1),
            [1, 2, 3].into_iter().map(VariableID::from).collect(),
        );
        instance
            .one_hot_constraint_collection
            .active_mut()
            .insert(OneHotConstraintID::from(1), oh);

        // Fix x1=0, x2=0 → unit propagation: x3 must be 1
        let state = v1::State::from(HashMap::from([(1, 0.0), (2, 0.0)]));
        instance.partial_evaluate(&state, ATol::default()).unwrap();

        assert_eq!(
            instance.decision_variables[&VariableID::from(3)].substituted_value(),
            Some(1.0)
        );
    }

    #[test]
    fn test_partial_evaluate_cascade_one_hot_sos1() {
        use crate::{
            DecisionVariable, OneHotConstraint, OneHotConstraintID, Sos1Constraint,
            Sos1ConstraintID,
        };
        use maplit::btreemap;

        // x1, x2 in OneHot; x2, x3 in SOS1
        // Fix x1=1 → OneHot propagates x2=0 → SOS1 shrinks (x2 removed)
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
            VariableID::from(3) => DecisionVariable::continuous(VariableID::from(3)),
        };
        let objective = Function::from(linear!(1) + linear!(2) + linear!(3));

        let mut instance = Instance::new(
            Sense::Minimize,
            objective,
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();

        let oh = OneHotConstraint::new(
            OneHotConstraintID::from(1),
            [1, 2].into_iter().map(VariableID::from).collect(),
        );
        instance
            .one_hot_constraint_collection
            .active_mut()
            .insert(OneHotConstraintID::from(1), oh);

        let sos1 = Sos1Constraint::new(
            Sos1ConstraintID::from(1),
            [2, 3].into_iter().map(VariableID::from).collect(),
        );
        instance
            .sos1_constraint_collection
            .active_mut()
            .insert(Sos1ConstraintID::from(1), sos1);

        // Fix x1=1 → OneHot: x2=0 → SOS1{x2,x3} shrinks to SOS1{x3}
        let state = v1::State::from(HashMap::from([(1, 1.0)]));
        instance.partial_evaluate(&state, ATol::default()).unwrap();

        // x2 should be fixed to 0 by propagation
        assert_eq!(
            instance.decision_variables[&VariableID::from(2)].substituted_value(),
            Some(0.0)
        );

        // OneHot consumed, SOS1 shrunk to just x3
        assert!(instance.one_hot_constraint_collection.active().is_empty());
        let sos1_active = instance.sos1_constraint_collection.active();
        assert_eq!(sos1_active.len(), 1);
        let remaining_sos1 = sos1_active.values().next().unwrap();
        assert_eq!(remaining_sos1.variables.len(), 1);
        assert!(remaining_sos1.variables.contains(&VariableID::from(3)));
    }

    #[test]
    fn test_partial_evaluate_indicator_promotion() {
        use crate::{constraint::Equality, DecisionVariable, IndicatorConstraintID};
        use maplit::btreemap;

        // x10 (indicator), x1, x2 (function variables)
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::continuous(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::continuous(VariableID::from(2)),
            VariableID::from(10) => DecisionVariable::binary(VariableID::from(10)),
        };
        let objective = Function::from(linear!(1) + linear!(2));

        let mut indicator_constraints = BTreeMap::new();
        indicator_constraints.insert(
            IndicatorConstraintID::from(100),
            crate::IndicatorConstraint::new(
                IndicatorConstraintID::from(100),
                VariableID::from(10),
                Equality::LessThanOrEqualToZero,
                Function::from(linear!(1) + linear!(2) + coeff!(-5.0)),
            ),
        );

        let instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(objective)
            .decision_variables(decision_variables)
            .constraints(BTreeMap::new())
            .indicator_constraints(indicator_constraints)
            .build()
            .unwrap();

        let mut instance = instance;

        // Fix x10=1 → indicator promoted to regular constraint
        let state = v1::State::from(HashMap::from([(10, 1.0)]));
        instance.partial_evaluate(&state, ATol::default()).unwrap();

        // Indicator constraint should be removed
        assert!(instance.indicator_constraint_collection.active().is_empty());
        assert_eq!(instance.indicator_constraint_collection.removed().len(), 1);

        // A new regular constraint should be added with ConstraintID(100)
        assert!(instance
            .constraint_collection
            .active()
            .contains_key(&ConstraintID::from(100)));
    }

    #[test]
    fn test_partial_evaluate_indicator_removed() {
        use crate::{constraint::Equality, DecisionVariable, IndicatorConstraintID};
        use maplit::btreemap;

        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::continuous(VariableID::from(1)),
            VariableID::from(10) => DecisionVariable::binary(VariableID::from(10)),
        };
        let objective = Function::from(linear!(1));

        let mut indicator_constraints = BTreeMap::new();
        indicator_constraints.insert(
            IndicatorConstraintID::from(1),
            crate::IndicatorConstraint::new(
                IndicatorConstraintID::from(1),
                VariableID::from(10),
                Equality::LessThanOrEqualToZero,
                Function::from(linear!(1) + coeff!(-5.0)),
            ),
        );

        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(objective)
            .decision_variables(decision_variables)
            .constraints(BTreeMap::new())
            .indicator_constraints(indicator_constraints)
            .build()
            .unwrap();

        // Fix x10=0 → indicator removed (vacuously satisfied)
        let state = v1::State::from(HashMap::from([(10, 0.0)]));
        instance.partial_evaluate(&state, ATol::default()).unwrap();

        assert!(instance.indicator_constraint_collection.active().is_empty());
        assert_eq!(instance.indicator_constraint_collection.removed().len(), 1);
        // No new regular constraint should be added
        assert!(instance.constraint_collection.active().is_empty());
    }
}
