use crate::{
    random::random_lp,
    v1::{
        instance::{Description, Sense},
        Function, Instance,
    },
};
use anyhow::{bail, Context, Result};
use approx::AbsDiffEq;
use proptest::prelude::*;
use rand::SeedableRng;
use std::collections::{BTreeMap, BTreeSet};

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

    pub fn defined_ids(&self) -> BTreeSet<u64> {
        self.decision_variables
            .iter()
            .map(|dv| dv.id)
            .collect::<BTreeSet<_>>()
    }

    pub fn check_decision_variables(&self) -> Result<()> {
        let used_ids = self.used_decision_variable_ids()?;
        let defined_ids = self.defined_ids();
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
                                    ..Default::default()
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

impl Arbitrary for Description {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_parameter: ()) -> Self::Strategy {
        (
            Option::<String>::arbitrary(),
            Option::<String>::arbitrary(),
            prop_oneof![Just(Vec::new()), proptest::collection::vec(".*", 1..3)],
            Option::<String>::arbitrary(),
        )
            .prop_map(|(name, description, authors, created_by)| Description {
                name,
                description,
                authors,
                created_by,
            })
            .boxed()
    }
}

/// Compare two instances as mathematical programming problems. This does not compare the metadata.
///
/// - This regards `min f` and `max -f` as the same problem.
/// - This cannot compare scaled constraints. For example, `2x + 3y <= 4` and `4x + 6y <= 8` are mathematically same,
///   but this regarded them as different problems.
///
impl AbsDiffEq for Instance {
    type Epsilon = f64;

    fn default_epsilon() -> Self::Epsilon {
        f64::default_epsilon()
    }

    fn abs_diff_eq(&self, other: &Self, epsilon: Self::Epsilon) -> bool {
        let (Some(f), Some(g)) = (&self.objective, &other.objective) else {
            // Return false if one of instance is invalid
            return false;
        };
        match (self.sense.try_into(), other.sense.try_into()) {
            (Ok(Sense::Minimize), Ok(Sense::Minimize))
            | (Ok(Sense::Maximize), Ok(Sense::Maximize)) => {
                if !f.abs_diff_eq(g, epsilon) {
                    return false;
                }
            }
            (Ok(Sense::Minimize), Ok(Sense::Maximize))
            | (Ok(Sense::Maximize), Ok(Sense::Minimize)) => {
                if !f.abs_diff_eq(&-g, epsilon) {
                    return false;
                }
            }
            _ => return false,
        }

        if self.constraints.len() != other.constraints.len() {
            return false;
        }
        // The constraints may not ordered in the same way
        let lhs = self
            .constraints
            .iter()
            .map(|c| (c.id, (c.equality, c.function())))
            .collect::<BTreeMap<_, _>>();
        for c in &other.constraints {
            if let (Some((eq, Ok(f))), Ok(g)) = (lhs.get(&c.id), c.function()) {
                if *eq != c.equality {
                    return false;
                }
                if !(*f).abs_diff_eq(g, epsilon) {
                    return false;
                }
            } else {
                return false;
            }
        }
        true
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