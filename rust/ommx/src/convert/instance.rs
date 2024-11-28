use crate::v1::{
    instance::{Description, Sense},
    Function, Instance, Parameter, ParametricInstance,
};
use anyhow::{bail, Result};
use approx::AbsDiffEq;
use num::Zero;
use proptest::prelude::*;
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
};

use super::{constraint::arbitrary_constraints, decision_variable::arbitrary_decision_variables};

impl Instance {
    pub fn objective(&self) -> Cow<Function> {
        match &self.objective {
            Some(f) => Cow::Borrowed(f),
            // Empty function is regarded as zero function
            None => Cow::Owned(Function::zero()),
        }
    }

    pub fn used_decision_variable_ids(&self) -> Result<BTreeSet<u64>> {
        let mut used_ids = self.objective().used_decision_variable_ids();
        for c in &self.constraints {
            used_ids.extend(c.function().used_decision_variable_ids());
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

    pub fn arbitrary_lp() -> BoxedStrategy<Self> {
        (0..10_usize, 0..10_usize, 0..=1_u32, 0..10_u64)
            .prop_flat_map(Self::arbitrary_with)
            .boxed()
    }

    pub fn penalty_method(self) -> ParametricInstance {
        let id_base = self.defined_ids().last().map(|id| id + 1).unwrap_or(0);
        let mut objective = self.objective().into_owned();
        let mut parameters = Vec::new();
        for (i, c) in self.constraints.into_iter().enumerate() {
            let parameter = Parameter {
                id: id_base + i as u64,
                name: Some("penalty".to_string()),
                subscripts: vec![c.id as i64],
                ..Default::default()
            };
            objective = objective + &parameter * c.function().into_owned();
            parameters.push(parameter);
        }
        ParametricInstance {
            description: self.description,
            objective: Some(objective),
            constraints: Vec::new(),
            decision_variables: self.decision_variables.clone(),
            sense: self.sense,
            parameters,
        }
    }
}

impl Arbitrary for Instance {
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
                    arbitrary_decision_variables(used_ids),
                    Option::<Description>::arbitrary(),
                    Sense::arbitrary(),
                )
                    .prop_map(
                        |(objective, constraints, decision_variables, description, sense)| {
                            Instance {
                                objective,
                                constraints,
                                decision_variables,
                                description,
                                sense: sense as i32,
                                ..Default::default()
                            }
                        },
                    )
            })
            .boxed()
    }

    fn arbitrary() -> Self::Strategy {
        (0..10_usize, 0..10_usize, 0..4_u32, 0..10_u64)
            .prop_flat_map(Self::arbitrary_with)
            .boxed()
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
        let f = self.objective();
        let g = other.objective();
        match (self.sense.try_into(), other.sense.try_into()) {
            (Ok(Sense::Minimize), Ok(Sense::Minimize))
            | (Ok(Sense::Maximize), Ok(Sense::Maximize)) => {
                if !f.abs_diff_eq(&g, epsilon) {
                    return false;
                }
            }
            (Ok(Sense::Minimize), Ok(Sense::Maximize))
            | (Ok(Sense::Maximize), Ok(Sense::Minimize)) => {
                if !f.abs_diff_eq(&-g.as_ref(), epsilon) {
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
            if let Some((eq, f)) = lhs.get(&c.id) {
                if *eq != c.equality {
                    return false;
                }
                if !f.abs_diff_eq(&c.function(), epsilon) {
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
        fn test_instance_arbitrary_any(instance in Instance::arbitrary()) {
            instance.check_decision_variables().unwrap();
        }
    }
}
