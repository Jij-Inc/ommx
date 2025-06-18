use super::*;
use crate::{
    v1::{Optimality, Relaxation, SampleSet, SampledDecisionVariable, Solution},
    ATol, Evaluate, VariableIDSet,
};
use anyhow::{anyhow, Result};
use std::collections::HashMap;

impl Evaluate for Instance {
    type Output = Solution;
    type SampledOutput = SampleSet;

    fn evaluate(&self, state: &v1::State, atol: ATol) -> Result<Self::Output> {
        let state = self
            .analyze_decision_variables()
            .populate(state.clone(), atol)?;

        let objective = self.objective.evaluate(&state, atol)?;

        let mut evaluated_constraints = Vec::new();
        let mut feasible_relaxed = true;
        for constraint in self.constraints.values() {
            let evaluated = constraint.evaluate(&state, atol)?;
            if !evaluated.feasible {
                feasible_relaxed = false;
            }
            evaluated_constraints.push(evaluated.into());
        }
        let mut feasible = feasible_relaxed;
        for constraint in self.removed_constraints.values() {
            let evaluated = constraint.evaluate(&state, atol)?;
            if !evaluated.feasible {
                feasible = false;
            }
            evaluated_constraints.push(evaluated.into());
        }

        let decision_variables = self
            .decision_variables
            .values()
            .map(|dv| {
                let id = dv.id().into_inner();
                let value = state.entries.get(&id).unwrap(); // Safe unwrap, as we populate the state with the decision variables
                let mut dv: v1::DecisionVariable = dv.clone().into();
                dv.substituted_value = Some(*value);
                dv
            })
            .collect();

        #[allow(deprecated)]
        Ok(Solution {
            state: Some(state.clone()),
            objective,
            evaluated_constraints,
            decision_variables,
            feasible,
            feasible_relaxed: Some(feasible_relaxed),
            // feasible_unrelaxed is deprecated, but we need to keep it for backward compatibility
            feasible_unrelaxed: feasible,
            // Optimality is only detectable in the context of a solver, and `State` does not store this information.
            optimality: Optimality::Unspecified as i32,
            // This field means that the solver relaxes the problem for some reason, and returns a solution for the relaxed problem.
            // The `removed_constraints` field do not relate to this. This is purely a solver-specific field.
            relaxation: Relaxation::Unspecified as i32,
        })
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

        let mut feasible_relaxed: HashMap<u64, bool> =
            samples.ids().map(|id| (*id, true)).collect();

        // Constraints
        let mut constraints = Vec::new();
        for c in self.constraints.values() {
            let evaluated = c.evaluate_samples(&samples, atol)?;
            for sample_id in evaluated.infeasible_ids(atol) {
                feasible_relaxed.insert(sample_id.into_inner(), false);
            }
            constraints.push(evaluated.into());
        }
        let mut feasible = feasible_relaxed.clone();
        for c in self.removed_constraints.values() {
            let v = c.evaluate_samples(&samples, atol)?;
            for sample_id in v.infeasible_ids(atol) {
                feasible.insert(sample_id.into_inner(), false);
            }
            constraints.push(v.into());
        }

        // Objective
        let objectives = self.objective().evaluate_samples(&samples, atol)?;

        // Reconstruct decision variable values
        let mut transposed = samples.transpose();
        let decision_variables: Vec<SampledDecisionVariable> = self
            .decision_variables
            .values()
            .map(|d| -> Result<_> {
                Ok(SampledDecisionVariable {
                    decision_variable: Some(d.clone().into()),
                    samples: transposed.remove(&d.id().into_inner()),
                })
            })
            .collect::<Result<_>>()?;

        Ok(SampleSet {
            decision_variables,
            objectives: Some(objectives),
            constraints,
            feasible_relaxed,
            feasible,
            sense: self.sense.into(),
            ..Default::default()
        })
    }

    fn partial_evaluate(&mut self, state: &v1::State, atol: ATol) -> Result<()> {
        for (id, value) in state.entries.iter() {
            let Some(dv) = self.decision_variables.get_mut(&VariableID::from(*id)) else {
                return Err(anyhow!("Unknown decision variable (ID={id}) in state."));
            };
            dv.substitute(*value, atol)?;
        }
        self.objective.partial_evaluate(state, atol)?;
        for constraint in self.constraints.values_mut() {
            constraint.partial_evaluate(state, atol)?;
        }
        for removed in self.removed_constraints.values_mut() {
            removed.partial_evaluate(state, atol)?;
        }
        self.decision_variable_dependency
            .partial_evaluate(state, atol)?;
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
    use ::approx::AbsDiffEq;
    use proptest::prelude::*;

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
            let ids: VariableIDSet = solution.state.unwrap().entries.keys().map(|id| VariableID::from(*id)).collect();
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
            prop_assert!(s1.state.unwrap().abs_diff_eq(&s2.state.unwrap(), ATol::default()));
        }
    }
}
