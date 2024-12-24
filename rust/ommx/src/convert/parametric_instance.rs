use super::{
    constraint::arbitrary_constraints, decision_variable::arbitrary_decision_variables,
    parameter::arbitrary_parameters,
};
use crate::{
    v1::{
        decision_variable::Kind,
        instance::{Description, Sense},
        Function, Instance, Parameters, ParametricInstance, State,
    },
    Evaluate,
};
use anyhow::{bail, Result};
use proptest::prelude::*;
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

    pub fn validate(&self) -> Result<()> {
        self.validate_ids()?;
        self.validate_constraint_ids()?;
        Ok(())
    }

    pub fn validate_ids(&self) -> Result<()> {
        let mut ids = BTreeSet::new();
        for dv in &self.decision_variables {
            if !ids.insert(dv.id) {
                bail!("Duplicate decision variable ID: {}", dv.id);
            }
        }
        for p in &self.parameters {
            if !ids.insert(p.id) {
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

impl Arbitrary for ParametricInstance {
    type Parameters = (usize, usize, u32, u64);
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(
        (num_constraints, num_terms, max_degree, max_id): Self::Parameters,
    ) -> Self::Strategy {
        (
            proptest::option::of(Function::arbitrary_with((num_terms, max_degree, max_id))),
            arbitrary_constraints(num_constraints, (num_terms, max_degree, max_id)),
        )
            .prop_flat_map(|(objective, constraints)| {
                let mut used_ids = objective
                    .as_ref()
                    .map(|f| f.used_decision_variable_ids())
                    .unwrap_or_default();
                for c in &constraints {
                    used_ids.extend(c.function().used_decision_variable_ids());
                }

                (
                    Just(objective),
                    Just(constraints),
                    arbitrary_split(used_ids),
                )
                    .prop_flat_map(
                        |(objective, constraints, (decision_variable_ids, parameter_ids))| {
                            (
                                Just(objective),
                                Just(constraints),
                                arbitrary_decision_variables(
                                    decision_variable_ids,
                                    Kind::arbitrary(),
                                ),
                                arbitrary_parameters(parameter_ids),
                                Option::<Description>::arbitrary(),
                                Sense::arbitrary(),
                            )
                                .prop_map(
                                    |(
                                        objective,
                                        constraints,
                                        decision_variables,
                                        parameters,
                                        description,
                                        sense,
                                    )| {
                                        ParametricInstance {
                                            objective,
                                            constraints,
                                            decision_variables,
                                            description,
                                            sense: sense as i32,
                                            parameters,
                                            ..Default::default()
                                        }
                                    },
                                )
                        },
                    )
            })
            .boxed()
    }
}

fn arbitrary_split(ids: BTreeSet<u64>) -> BoxedStrategy<(BTreeSet<u64>, BTreeSet<u64>)> {
    let flips = proptest::collection::vec(bool::arbitrary(), ids.len());
    flips
        .prop_map(move |flips| {
            let mut used_ids = BTreeSet::new();
            let mut defined_ids = BTreeSet::new();
            for (flip, id) in flips.into_iter().zip(ids.iter()) {
                if flip {
                    used_ids.insert(*id);
                } else {
                    defined_ids.insert(*id);
                }
            }
            (used_ids, defined_ids)
        })
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::abs_diff_eq;

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
