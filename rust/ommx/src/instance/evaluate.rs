use super::*;
use crate::{ATol, Evaluate, VariableIDSet};
use anyhow::{anyhow, Result};
use fnv::FnvHashMap;
use std::collections::BTreeMap;

impl Evaluate for Instance {
    type Output = crate::Solution;
    type SampledOutput = crate::SampleSet;

    fn evaluate(&self, state: &v1::State, atol: ATol) -> Result<Self::Output> {
        let state = self
            .analyze_decision_variables()
            .populate(state.clone(), atol)?;

        let objective = self.objective.evaluate(&state, atol)?;

        let mut evaluated_constraints = BTreeMap::default();
        for constraint in self.constraint_collection.active().values() {
            let evaluated = constraint.evaluate(&state, atol)?;
            evaluated_constraints.insert(evaluated.id, evaluated);
        }
        for constraint in self.constraint_collection.removed().values() {
            let evaluated = constraint.evaluate(&state, atol)?;
            evaluated_constraints.insert(evaluated.id, evaluated);
        }

        let mut decision_variables = BTreeMap::default();
        for dv in self.decision_variables.values() {
            let evaluated_dv = dv.evaluate(&state, atol)?;
            decision_variables.insert(*evaluated_dv.id(), evaluated_dv);
        }

        let mut evaluated_indicator_constraints = BTreeMap::default();
        for ic in self.indicator_constraint_collection.active().values() {
            let evaluated = ic.evaluate(&state, atol)?;
            evaluated_indicator_constraints.insert(evaluated.id, evaluated);
        }
        for ic in self.indicator_constraint_collection.removed().values() {
            let evaluated = ic.evaluate(&state, atol)?;
            evaluated_indicator_constraints.insert(evaluated.id, evaluated);
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
                .evaluated_constraints(evaluated_constraints)
                .evaluated_indicator_constraints(evaluated_indicator_constraints)
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

        let mut feasible_relaxed: FnvHashMap<u64, bool> =
            samples.ids().map(|id| (*id, true)).collect();

        // Constraints
        let mut constraints = Vec::new();
        for c in self.constraint_collection.active().values() {
            let evaluated = c.evaluate_samples(&samples, atol)?;
            for sample_id in evaluated.infeasible_ids(atol) {
                feasible_relaxed.insert(sample_id.into_inner(), false);
            }
            constraints.push(evaluated);
        }
        let mut feasible = feasible_relaxed.clone();
        for c in self.constraint_collection.removed().values() {
            let v = c.evaluate_samples(&samples, atol)?;
            for sample_id in v.infeasible_ids(atol) {
                feasible.insert(sample_id.into_inner(), false);
            }
            constraints.push(v);
        }

        // Indicator constraints
        let mut indicator_constraints = Vec::new();
        for ic in self.indicator_constraint_collection.active().values() {
            let evaluated = ic.evaluate_samples(&samples, atol)?;
            indicator_constraints.push(evaluated);
        }
        for ic in self.indicator_constraint_collection.removed().values() {
            let v = ic.evaluate_samples(&samples, atol)?;
            indicator_constraints.push(v);
        }

        // Objective
        let objectives = self.objective().evaluate_samples(&samples, atol)?;

        // Reconstruct decision variable values
        let mut decision_variables = std::collections::BTreeMap::new();
        for dv in self.decision_variables.values() {
            let sampled_dv = dv.evaluate_samples(&samples, atol)?;
            decision_variables.insert(dv.id(), sampled_dv);
        }

        // Reconstruct constraint values
        let mut constraints_map = std::collections::BTreeMap::new();
        for constraint in constraints {
            constraints_map.insert(constraint.id, constraint);
        }

        // Reconstruct named function values
        let mut named_functions = std::collections::BTreeMap::new();
        for (id, named_function) in self.named_functions.iter() {
            let sampled_named_function = named_function.evaluate_samples(&samples, atol)?;
            named_functions.insert(*id, sampled_named_function);
        }

        // Reconstruct indicator constraint values
        let mut indicator_constraints_map = std::collections::BTreeMap::new();
        for ic in indicator_constraints {
            indicator_constraints_map.insert(ic.id, ic);
        }

        Ok(crate::SampleSet::builder()
            .decision_variables(decision_variables)
            .objectives(objectives.try_into()?)
            .constraints(constraints_map)
            .indicator_constraints(indicator_constraints_map)
            .named_functions(named_functions)
            .sense(self.sense)
            .build()?)
    }

    fn partial_evaluate(&mut self, state: &v1::State, atol: ATol) -> Result<()> {
        // First, apply constraint hints to potentially update the state
        let updated_state = self
            .constraint_hints
            .partial_evaluate(state.clone(), atol)?;

        // Then proceed with the regular partial evaluation using the updated state
        for (id, value) in updated_state.entries.iter() {
            let Some(dv) = self.decision_variables.get_mut(&VariableID::from(*id)) else {
                return Err(anyhow!("Unknown decision variable (ID={id}) in state."));
            };
            dv.substitute(*value, atol)?;
        }
        self.objective.partial_evaluate(&updated_state, atol)?;
        // Only partial_evaluate active constraints.
        // Removed constraints are not evaluated here; they will be substituted
        // when restored via `restore_constraint`.
        for constraint in self.constraint_collection.active_mut().values_mut() {
            constraint.partial_evaluate(&updated_state, atol)?;
        }
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
    use crate::{coeff, constraint_hints::OneHot, linear};
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

    #[test]
    fn test_partial_evaluate_with_constraint_hints() {
        use crate::DecisionVariable;
        use maplit::btreemap;

        // Create an instance with OneHot constraint
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
            VariableID::from(3) => DecisionVariable::binary(VariableID::from(3)),
        };

        // Objective: minimize x1 + x2 + x3
        let objective = Function::from(linear!(1) + linear!(2) + linear!(3));

        // Create a OneHot constraint for variables 1, 2, 3
        let mut constraint_hints = crate::constraint_hints::ConstraintHints::default();
        constraint_hints.one_hot_constraints.push(OneHot {
            id: ConstraintID::from(100),
            variables: vec![
                VariableID::from(1),
                VariableID::from(2),
                VariableID::from(3),
            ]
            .into_iter()
            .collect(),
        });
        let mut instance = Instance::new(
            Sense::Minimize,
            objective,
            decision_variables,
            BTreeMap::new(), // No regular constraints
        )
        .unwrap();
        instance.constraint_hints = constraint_hints;

        // Create initial state where variable 2 is fixed to 1
        let initial_state = v1::State::from(HashMap::from([(2, 1.0)]));

        // Apply partial evaluate
        instance
            .partial_evaluate(&initial_state, ATol::default())
            .unwrap();

        // After partial evaluation, due to OneHot constraint propagation:
        // - Variable 2 remains fixed to 1
        // - Variables 1 and 3 should be fixed to 0

        // Verify by evaluating with empty state (all fixed variables should be substituted)
        let empty_state = v1::State::default();
        let solution = instance.evaluate(&empty_state, ATol::default()).unwrap();

        // Check that the state contains all three variables with correct values
        assert_eq!(solution.state().entries.get(&1), Some(&0.0));
        assert_eq!(solution.state().entries.get(&2), Some(&1.0));
        assert_eq!(solution.state().entries.get(&3), Some(&0.0));

        // The objective value should be 1 (only x2 = 1)
        assert_eq!(*solution.objective(), 1.0);
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
