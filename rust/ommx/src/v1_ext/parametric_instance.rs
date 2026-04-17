use crate::{
    v1::{Instance, Parameters, ParametricInstance, State},
    Evaluate,
};
use anyhow::{bail, Result};
use std::collections::BTreeSet;

impl From<Instance> for ParametricInstance {
    fn from(
        Instance {
            description,
            objective,
            constraints,
            decision_variables,
            sense,
            constraint_hints,
            removed_constraints,
            parameters: _, // Drop previous parameters
            decision_variable_dependency,
            named_functions,
        }: Instance,
    ) -> Self {
        Self {
            description,
            objective,
            constraints,
            decision_variables,
            sense,
            parameters: Default::default(),
            constraint_hints,
            removed_constraints,
            decision_variable_dependency,
            named_functions,
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
    pub fn with_parameters(
        mut self,
        parameters: Parameters,
        atol: crate::ATol,
    ) -> Result<Instance> {
        let required_ids: BTreeSet<u64> = self.parameters.iter().map(|p| p.id).collect();
        let given_ids: BTreeSet<u64> = parameters.entries.keys().cloned().collect();
        if !required_ids.is_subset(&given_ids) {
            for ids in required_ids.difference(&given_ids) {
                let parameter = self.parameters.iter().find(|p| p.id == *ids).unwrap();
                log::error!("Missing parameter: {parameter:?}");
            }
            bail!(
                "Missing parameters: Required IDs {:?}, got {:?}",
                required_ids,
                given_ids
            );
        }

        let state = State::from(parameters.clone());
        if let Some(f) = self.objective.as_mut() {
            f.partial_evaluate(&state, atol)?;
        }
        for constraint in self.constraints.iter_mut() {
            constraint.partial_evaluate(&state, atol)?;
        }

        Ok(Instance {
            description: self.description,
            objective: self.objective,
            constraints: self.constraints,
            decision_variables: self.decision_variables,
            sense: self.sense,
            parameters: Some(parameters),
            constraint_hints: self.constraint_hints,
            removed_constraints: self.removed_constraints,
            decision_variable_dependency: self.decision_variable_dependency,
            named_functions: self.named_functions,
        })
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
            let converted_instance: Instance = parametric_instance.with_parameters(Parameters::default(), crate::ATol::default()).unwrap();
            prop_assert_eq!(&converted_instance.parameters, &Some(Parameters::default()));
            prop_assert!(
                abs_diff_eq!(instance, converted_instance),
                "\nLeft : {:?}\nRight: {:?}", instance, converted_instance
            );
        }

    }
}
