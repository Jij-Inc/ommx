use crate::{
    random::random_lp,
    v1::{
        instance::{Description, Sense},
        Constraint, DecisionVariable, Equality, Function, Instance,
    },
};
use proptest::prelude::*;
use rand::SeedableRng;

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
        InstanceParameter::LP {
            num_constraints: 5,
            num_variables: 7,
        }
    }
}

impl Arbitrary for Instance {
    type Parameters = InstanceParameter;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(parameter: InstanceParameter) -> Self::Strategy {
        // The instance yielded from strategy must depends only on the parameter deterministically.
        // Thus we should not use `thread_rng` here.
        let mut rng = rand_xoshiro::Xoshiro256StarStar::seed_from_u64(0);
        match parameter {
            InstanceParameter::LP {
                num_constraints,
                num_variables,
            } => Just(random_lp(&mut rng, num_variables, num_constraints)).boxed(),
            InstanceParameter::Any {
                num_constraints,
                num_terms,
                max_id,
                max_degree,
            } => {
                let objective = Function::arbitrary_with((num_terms, max_degree, max_id));
                let constraints = proptest::collection::vec(
                    Constraint::arbitrary_with((num_terms, max_degree, max_id)),
                    num_constraints,
                );
                let constraint_ids = prop_oneof![
                    // continuous case
                    Just((0..(num_constraints as u64)).collect::<Vec<u64>>()).prop_shuffle(),
                    // discrete case
                    Just((0..(3 * num_constraints as u64)).collect::<Vec<u64>>()).prop_shuffle(),
                ];
                let constraints = (constraints, constraint_ids).prop_map(|(mut c, id)| {
                    for (id, c) in id.iter().zip(c.iter_mut()) {
                        c.id = *id;
                    }
                    c
                });
                (objective, constraints)
                    .prop_flat_map(|(objective, constraints)| {
                        let mut used_ids = objective.used_decision_variable_ids();
                        for c in &constraints {
                            used_ids
                                .extend(c.function.as_ref().unwrap().used_decision_variable_ids());
                        }
                        let decision_variables = (
                            proptest::collection::vec(
                                DecisionVariable::arbitrary(),
                                used_ids.len(),
                            ),
                            Just(used_ids),
                        )
                            .prop_map(|(mut dvs, used_ids)| {
                                for (dv, id) in dvs.iter_mut().zip(used_ids.iter()) {
                                    dv.id = *id;
                                }
                                dvs
                            });
                        (
                            Just(objective),
                            Just(constraints),
                            decision_variables,
                            Option::<Description>::arbitrary(),
                            prop_oneof![Just(Sense::Minimize as i32), Just(Sense::Maximize as i32)],
                        )
                            .prop_map(
                                |(
                                    objective,
                                    constraints,
                                    decision_variables,
                                    description,
                                    sense,
                                )| {
                                    Instance {
                                        objective: Some(objective),
                                        constraints,
                                        decision_variables,
                                        description,
                                        sense,
                                    }
                                },
                            )
                    })
                    .boxed()
            }
        }
    }
}

impl Arbitrary for Constraint {
    type Parameters = <Function as Arbitrary>::Parameters;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(parameters: Self::Parameters) -> Self::Strategy {
        let function = Function::arbitrary_with(parameters);
        let equality = prop_oneof![
            Just(Equality::EqualToZero),
            Just(Equality::LessThanOrEqualToZero)
        ];
        (function, equality)
            .prop_map(|(function, equality)| Constraint {
                id: 0, // ID should be changed when creating an instance
                function: Some(function),
                equality: equality as i32,
                ..Default::default()
            })
            .boxed()
    }

    fn arbitrary() -> Self::Strategy {
        (0..10_usize, 0..5_usize, 0..10_u64)
            .prop_flat_map(Self::arbitrary_with)
            .boxed()
    }
}
