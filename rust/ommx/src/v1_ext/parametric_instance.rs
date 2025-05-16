use crate::{
    v1::{Function, Instance, Parameters, ParametricInstance, State},
    Evaluate, VariableIDSet,
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
            constraint_hints,
            removed_constraints,
            parameters: _, // Drop previous parameters
            decision_variable_dependency,
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
            f.partial_evaluate(&state, 1e-9)?;
        }
        for constraint in self.constraints.iter_mut() {
            constraint.partial_evaluate(&state, 1e-9)?;
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
        })
    }

    pub fn objective(&self) -> Cow<Function> {
        match &self.objective {
            Some(f) => Cow::Borrowed(f),
            None => Cow::Owned(Function::default()),
        }
    }

    /// Used decision variable and parameter IDs in the objective and constraints.
    pub fn used_ids(&self) -> Result<VariableIDSet> {
        let mut used_ids = self.objective().required_ids();
        for c in &self.constraints {
            used_ids.extend(c.function().required_ids());
        }
        Ok(used_ids)
    }

    /// Defined decision variable IDs. These IDs may not be used in the objective and constraints.
    pub fn defined_decision_variable_ids(&self) -> VariableIDSet {
        self.decision_variables
            .iter()
            .map(|dv| dv.id.into())
            .collect()
    }

    /// Defined parameter IDs. These IDs may not be used in the objective and constraints.
    pub fn defined_parameter_ids(&self) -> VariableIDSet {
        self.parameters.iter().map(|p| p.id.into()).collect()
    }

    pub fn validate(&self) -> Result<()> {
        self.validate_ids()?;
        self.validate_constraint_ids()?;
        Ok(())
    }

    pub fn validate_ids(&self) -> Result<()> {
        let mut ids = VariableIDSet::default();
        for dv in &self.decision_variables {
            if !ids.insert(dv.id.into()) {
                bail!("Duplicate decision variable ID: {}", dv.id);
            }
        }
        for p in &self.parameters {
            if !ids.insert(p.id.into()) {
                bail!("Duplicate parameter ID: {}", p.id);
            }
        }
        let used_ids = self.used_ids()?;
        if !used_ids.is_subset(&ids) {
            let sub = used_ids.difference(&ids).collect::<BTreeSet<_>>();
            bail!("Undefined ID is used: {:?}", sub);
        }
        Ok(())
    }

    pub fn validate_constraint_ids(&self) -> Result<()> {
        let mut ids = BTreeSet::new();
        for c in &self.constraints {
            if !ids.insert(c.id) {
                bail!("Duplicate constraint ID: {}", c.id);
            }
        }
        for c in &self.removed_constraints {
            if let Some(c) = c.constraint.as_ref() {
                if !ids.insert(c.id) {
                    bail!("Duplicate removed constraint ID: {}", c.id);
                }
            }
        }
        Ok(())
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

        #[test]
        fn validate(pi in ParametricInstance::arbitrary()) {
            pi.validate().unwrap();
        }
    }
}
