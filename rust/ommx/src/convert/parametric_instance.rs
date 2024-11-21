use crate::{
    v1::{Instance, Parameters, ParametricInstance, State},
    Evaluate,
};
use anyhow::{bail, Context, Result};
use std::collections::BTreeSet;

impl From<Instance> for ParametricInstance {
    fn from(
        Instance {
            description,
            objective,
            constraints,
            decision_variables,
            sense,
            parameters: _, // Drop previous parameters
        }: Instance,
    ) -> Self {
        Self {
            description,
            objective,
            constraints,
            decision_variables,
            sense,
            parameters: Default::default(),
        }
    }
}

impl From<State> for Parameters {
    fn from(State { entries }: State) -> Self {
        Self { entries }
    }
}

impl From<Parameters> for State {
    fn from(Parameters { entries }: Parameters) -> Self {
        Self { entries }
    }
}

impl ParametricInstance {
    /// Create a new [Instance] with the given parameters.
    pub fn with_parameters(mut self, parameters: Parameters) -> Result<Instance> {
        let required_ids: BTreeSet<u64> = self.parameters.iter().map(|p| p.id).collect();
        let given_ids: BTreeSet<u64> = parameters.entries.keys().cloned().collect();
        if !required_ids.is_subset(&given_ids) {
            for ids in required_ids.difference(&given_ids) {
                let parameter = self.parameters.iter().find(|p| p.id == *ids).unwrap();
                log::error!("Missing parameter: {:?}", parameter);
            }
            bail!(
                "Missing parameters: Required IDs {:?}, got {:?}",
                required_ids,
                given_ids
            );
        }

        let state = State::from(parameters.clone());
        self.objective
            .as_mut()
            .context("Objective function of ParametricInstance is empty")?
            .partial_evaluate(&state)?;
        for constraint in self.constraints.iter_mut() {
            constraint.partial_evaluate(&state)?;
        }

        Ok(Instance {
            description: self.description,
            objective: self.objective,
            constraints: self.constraints,
            decision_variables: self.decision_variables,
            sense: self.sense,
            parameters: Some(parameters),
        })
    }
}
