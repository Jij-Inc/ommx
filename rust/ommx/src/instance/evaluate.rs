use super::*;
use crate::{
    v1::{Optimality, Relaxation, SampleSet, SampledDecisionVariable, Solution},
    Evaluate, VariableIDSet,
};
use anyhow::{anyhow, Result};
use std::collections::HashMap;

impl Evaluate for Instance {
    type Output = Solution;
    type SampledOutput = SampleSet;

    fn evaluate(&self, state: &v1::State, atol: f64) -> Result<Self::Output> {
        let state = self
            .analyze_decision_variables()
            .populate(state.clone(), atol)?;

        let objective = self.objective.evaluate(&state, atol)?;

        let mut evaluated_constraints = Vec::new();
        let mut feasible_relaxed = true;
        for constraint in self.constraints.values() {
            let evaluated = constraint.evaluate(&state, atol)?;
            if !evaluated.is_feasible(atol)? {
                feasible_relaxed = false;
            }
            evaluated_constraints.push(evaluated);
        }
        let mut feasible = feasible_relaxed;
        for constraint in self.removed_constraints.values() {
            let evaluated = constraint.evaluate(&state, atol)?;
            if !evaluated.is_feasible(atol)? {
                feasible = false;
            }
            evaluated_constraints.push(evaluated);
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
            // Optimality is only detecable in the context of a solver, and `State` does not store this information.
            optimality: Optimality::Unspecified as i32,
            // This field means that the solver relaxes the problem for some reason, and returns a solution for the relaxed problem.
            // The `removed_constraints` field do not relate to this. This is purely a solver-specific field.
            relaxation: Relaxation::Unspecified as i32,
        })
    }

    fn evaluate_samples(&self, samples: &v1::Samples, atol: f64) -> Result<Self::SampledOutput> {
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
            for (sample_id, feasible_) in evaluated.is_feasible(atol)? {
                if !feasible_ {
                    feasible_relaxed.insert(sample_id, false);
                }
            }
            constraints.push(evaluated);
        }
        let mut feasible = feasible_relaxed.clone();
        for c in self.removed_constraints.values() {
            let v = c.evaluate_samples(&samples, atol)?;
            for (sample_id, feasible_) in v.is_feasible(atol)? {
                if !feasible_ {
                    feasible.insert(sample_id, false);
                }
            }
            constraints.push(v);
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

    fn partial_evaluate(&mut self, state: &v1::State, atol: f64) -> Result<()> {
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
        for f in self.decision_variable_dependency.values_mut() {
            f.partial_evaluate(state, atol)?;
        }
        Ok(())
    }

    fn required_ids(&self) -> VariableIDSet {
        self.analyze_decision_variables().used().clone()
    }
}
