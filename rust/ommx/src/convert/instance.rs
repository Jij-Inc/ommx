use crate::{
    random::random_lp,
    v1::{
        instance::{Description, Sense},
        Function, Instance,
    },
};
use anyhow::{bail, Context, Result};
use proptest::prelude::*;
use rand::SeedableRng;
use std::collections::BTreeSet;

use super::{constraint::arbitrary_constraints, decision_variable::arbitrary_decision_variables};

impl Instance {
    pub fn objective(&self) -> Result<&Function> {
        self.objective
            .as_ref()
            .context("Instance does not contain objective function")
    }

    pub fn used_decision_variable_ids(&self) -> Result<BTreeSet<u64>> {
        let mut used_ids = self.objective()?.used_decision_variable_ids();
        for c in &self.constraints {
            used_ids.extend(c.function()?.used_decision_variable_ids());
        }
        Ok(used_ids)
    }

    pub fn check_decision_variables(&self) -> Result<()> {
        let used_ids = self.used_decision_variable_ids()?;
        let defined_ids = self
            .decision_variables
            .iter()
            .map(|dv| dv.id)
            .collect::<BTreeSet<_>>();
        if !used_ids.is_subset(&defined_ids) {
            let undefined_ids = used_ids.difference(&defined_ids).collect::<Vec<_>>();
            bail!("Undefined decision variable IDs: {:?}", undefined_ids);
        }
        Ok(())
    }
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum InstanceParameter {
    Any {
        num_constraints: usize,
        num_terms: usize,
        max_id: u64,
        max_degree: usize,
    },
    LP {
        num_constraints: usize,
        num_variables: usize,
    },
    // FIXME: Add more instance types
}

impl Default for InstanceParameter {
    fn default() -> Self {
        InstanceParameter::Any {
            num_constraints: 3,
            num_terms: 3,
            max_id: 5,
            max_degree: 3,
        }
    }
}

impl Arbitrary for Instance {
    type Parameters = InstanceParameter;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(parameter: InstanceParameter) -> Self::Strategy {
        match parameter {
            InstanceParameter::LP {
                num_constraints,
                num_variables,
            } => {
                // The instance yielded from strategy must depends only on the parameter deterministically.
                // Thus we should not use `thread_rng` here.
                let mut rng = rand_xoshiro::Xoshiro256StarStar::seed_from_u64(0);
                Just(random_lp(&mut rng, num_variables, num_constraints)).boxed()
            }
            InstanceParameter::Any {
                num_constraints,
                num_terms,
                max_id,
                max_degree,
            } => (
                Function::arbitrary_with((num_terms, max_degree, max_id)),
                arbitrary_constraints(num_constraints, (num_terms, max_degree, max_id)),
            )
                .prop_flat_map(|(objective, constraints)| {
                    let mut used_ids = objective.used_decision_variable_ids();
                    for c in &constraints {
                        used_ids.extend(c.function().unwrap().used_decision_variable_ids());
                    }
                    (
                        Just(objective),
                        Just(constraints),
                        arbitrary_decision_variables(used_ids),
                        Option::<Description>::arbitrary(),
                        Sense::arbitrary(),
                    )
                        .prop_map(
                            |(objective, constraints, decision_variables, description, sense)| {
                                Instance {
                                    objective: Some(objective),
                                    constraints,
                                    decision_variables,
                                    description,
                                    sense: sense as i32,
                                }
                            },
                        )
                })
                .boxed(),
        }
    }
}

impl Arbitrary for Sense {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_parameter: ()) -> Self::Strategy {
        prop_oneof![Just(Sense::Minimize), Just(Sense::Maximize)].boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    proptest! {
        #[test]
        fn test_instance_arbitrary_lp(
            instance in Instance::arbitrary_with(
                InstanceParameter::LP {
                    num_constraints: 5,
                    num_variables: 3
                }
            )
        ) {
            instance.check_decision_variables().unwrap();
        }

        #[test]
        fn test_instance_arbitrary_any(
            instance in Instance::arbitrary_with(
                InstanceParameter::Any {
                    num_constraints: 3,
                    num_terms: 3,
                    max_id: 5,
                    max_degree: 3
                }
            )
        ) {
            instance.check_decision_variables().unwrap();
        }
    }
}
