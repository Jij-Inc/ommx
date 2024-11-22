use crate::{
    v1::{Function, Instance, Parameters, ParametricInstance, State},
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

    pub fn objective(&self) -> Result<&Function> {
        self.objective
            .as_ref()
            .context("Objective function of ParametricInstance is empty")
    }

    pub fn used_ids(&self) -> Result<BTreeSet<u64>> {
        let mut used_ids = self.objective()?.used_decision_variable_ids();
        for c in &self.constraints {
            used_ids.extend(c.function()?.used_decision_variable_ids());
        }
        Ok(used_ids)
    }

    pub fn defined_ids(&self) -> BTreeSet<u64> {
        self.decision_variables
            .iter()
            .map(|dv| dv.id)
            .collect::<BTreeSet<_>>()
    }

    pub fn parameter_ids(&self) -> BTreeSet<u64> {
        self.parameters.iter().map(|p| p.id).collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::convert::instance::InstanceParameter;

    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_parametric_instance_conversion(instance in Instance::arbitrary_with(InstanceParameter::Any {
            num_constraints: 2,
            num_terms: 2,
            max_id: 5,
            max_degree: 2
        })) {
            let parametric_instance: ParametricInstance = instance.clone().into();
            let converted_instance: Instance = parametric_instance.with_parameters(Parameters::default()).unwrap();
            prop_assert_eq!(instance, converted_instance);
        }
    }
}
