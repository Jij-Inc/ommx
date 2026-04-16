use super::*;
use crate::{ATol, Evaluate, VariableIDSet};
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
        let updated_state = state.clone();

        // Validate that no indicator variable is being partially evaluated.
        // This check must happen before any mutation to ensure the Instance
        // is not left in an inconsistent state on error.
        for ic in self.indicator_constraint_collection.active().values() {
            if updated_state
                .entries
                .contains_key(&ic.indicator_variable.into_inner())
            {
                anyhow::bail!(
                    "Cannot partially evaluate indicator variable {:?} of indicator constraint {:?}. \
                     Fixing an indicator variable would change the constraint type.",
                    ic.indicator_variable,
                    ic.id
                );
            }
        }

        // Validate that no one-hot or SOS1 variable is being partially evaluated.
        for oh in self.one_hot_constraint_collection.active().values() {
            for var_id in &oh.variables {
                if updated_state.entries.contains_key(&var_id.into_inner()) {
                    anyhow::bail!(
                        "Cannot partially evaluate variable {:?} of one-hot constraint {:?}. \
                         Fixing a one-hot variable would change the constraint type.",
                        var_id,
                        oh.id
                    );
                }
            }
        }
        for sos1 in self.sos1_constraint_collection.active().values() {
            for var_id in &sos1.variables {
                if updated_state.entries.contains_key(&var_id.into_inner()) {
                    anyhow::bail!(
                        "Cannot partially evaluate variable {:?} of SOS1 constraint {:?}. \
                         Fixing a SOS1 variable would change the constraint type.",
                        var_id,
                        sos1.id
                    );
                }
            }
        }

        // Then proceed with the regular partial evaluation using the updated state
        for (id, value) in updated_state.entries.iter() {
            let Some(dv) = self.decision_variables.get_mut(&VariableID::from(*id)) else {
                return Err(anyhow!("Unknown decision variable (ID={id}) in state."));
            };
            dv.substitute(*value, atol)?;
        }
        self.objective.partial_evaluate(&updated_state, atol)?;
        self.constraint_collection
            .partial_evaluate(&updated_state, atol)?;
        // Indicator variable check already passed above, so this only
        // partial_evaluates the function parts of indicator constraints.
        self.indicator_constraint_collection
            .partial_evaluate(&updated_state, atol)?;
        for named_function in self.named_functions.values_mut() {
            named_function.partial_evaluate(&updated_state, atol)?;
        }
        self.decision_variable_dependency
            .partial_evaluate(&updated_state, atol)?;
        Ok(())
    }

    fn required_ids(&self) -> VariableIDSet {
        self.analyze_decision_variables().used().clone()
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
}
