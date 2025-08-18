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
        for constraint in self.constraints.values() {
            let evaluated = constraint.evaluate(&state, atol)?;
            evaluated_constraints.insert(*evaluated.id(), evaluated);
        }
        for constraint in self.removed_constraints.values() {
            let evaluated = constraint.evaluate(&state, atol)?;
            evaluated_constraints.insert(*evaluated.id(), evaluated);
        }

        let mut decision_variables = BTreeMap::default();
        for dv in self.decision_variables.values() {
            let evaluated_dv = dv.evaluate(&state, atol)?;
            decision_variables.insert(*evaluated_dv.id(), evaluated_dv);
        }

        let sense = self.sense();

        let solution =
            crate::Solution::new(objective, evaluated_constraints, decision_variables, sense);

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
        for c in self.constraints.values() {
            let evaluated = c.evaluate_samples(&samples, atol)?;
            for sample_id in evaluated.infeasible_ids(atol) {
                feasible_relaxed.insert(sample_id.into_inner(), false);
            }
            constraints.push(evaluated);
        }
        let mut feasible = feasible_relaxed.clone();
        for c in self.removed_constraints.values() {
            let v = c.evaluate_samples(&samples, atol)?;
            for sample_id in v.infeasible_ids(atol) {
                feasible.insert(sample_id.into_inner(), false);
            }
            constraints.push(v);
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
            constraints_map.insert(*constraint.id(), constraint);
        }

        Ok(crate::SampleSet::new(
            decision_variables,
            objectives.try_into()?,
            constraints_map,
            self.sense,
        )?)
    }

    fn partial_evaluate(&mut self, state: &v1::State, atol: ATol) -> Result<()> {
        // First, apply constraint hints to potentially update the state
        let updated_state = self.constraint_hints.partial_evaluate(state.clone(), atol)?;
        
        // Then proceed with the regular partial evaluation using the updated state
        for (id, value) in updated_state.entries.iter() {
            let Some(dv) = self.decision_variables.get_mut(&VariableID::from(*id)) else {
                return Err(anyhow!("Unknown decision variable (ID={id}) in state."));
            };
            dv.substitute(*value, atol)?;
        }
        self.objective.partial_evaluate(&updated_state, atol)?;
        for constraint in self.constraints.values_mut() {
            constraint.partial_evaluate(&updated_state, atol)?;
        }
        for removed in self.removed_constraints.values_mut() {
            removed.partial_evaluate(&updated_state, atol)?;
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
    use crate::{constraint_hints::OneHot, linear};
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
        instance.partial_evaluate(&initial_state, ATol::default()).unwrap();

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
}
