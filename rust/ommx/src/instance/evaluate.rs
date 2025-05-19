use super::*;
use crate::{
    v1::{Optimality, Relaxation, SampleSet, Solution},
    Evaluate, VariableIDSet,
};
use anyhow::Result;

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
                let id = dv.id.into_inner();
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
        todo!()
    }

    fn partial_evaluate(&mut self, state: &v1::State, atol: f64) -> Result<()> {
        todo!()
    }

    fn required_ids(&self) -> VariableIDSet {
        self.analyze_decision_variables().used().clone()
    }
}
