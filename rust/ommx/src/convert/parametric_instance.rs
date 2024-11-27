use crate::{
    v1::{Function, Instance, Parameters, ParametricInstance, State},
    Evaluate,
};
use anyhow::{bail, Result};
use std::{borrow::Cow, collections::BTreeSet};

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
        if let Some(f) = self.objective.as_mut() {
            f.partial_evaluate(&state)?;
        }
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

    pub fn objective(&self) -> Cow<Function> {
        match &self.objective {
            Some(f) => Cow::Borrowed(f),
            None => Cow::Owned(Function::default()),
        }
    }

    /// Used decision variable and parameter IDs in the objective and constraints.
    pub fn used_ids(&self) -> Result<BTreeSet<u64>> {
        let mut used_ids = self.objective().used_decision_variable_ids();
        for c in &self.constraints {
            used_ids.extend(c.function().used_decision_variable_ids());
        }
        Ok(used_ids)
    }

    /// Defined decision variable IDs. These IDs may not be used in the objective and constraints.
    pub fn defined_decision_variable_ids(&self) -> BTreeSet<u64> {
        self.decision_variables
            .iter()
            .map(|dv| dv.id)
            .collect::<BTreeSet<_>>()
    }

    /// Defined parameter IDs. These IDs may not be used in the objective and constraints.
    pub fn defined_parameter_ids(&self) -> BTreeSet<u64> {
        self.parameters.iter().map(|p| p.id).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::abs_diff_eq;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_parametric_instance_conversion(instance in Instance::arbitrary()) {
            let parametric_instance: ParametricInstance = instance.clone().into();
            let converted_instance: Instance = parametric_instance.with_parameters(Parameters::default()).unwrap();
            prop_assert_eq!(&converted_instance.parameters, &Some(Parameters::default()));
            prop_assert!(
                abs_diff_eq!(instance, converted_instance, epsilon = 1e-10),
                "\nLeft : {:?}\nRight: {:?}", instance, converted_instance
            );
        }
    }
}
