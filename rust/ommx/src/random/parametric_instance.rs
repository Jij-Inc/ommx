use crate::{
    random::{
        arbitrary_constraints, arbitrary_decision_variables, arbitrary_parameters,
        InstanceParameters,
    },
    v1::{
        instance::{Description, Sense},
        Function, ParametricInstance,
    },
};
use proptest::prelude::*;
use std::collections::BTreeSet;

impl Arbitrary for ParametricInstance {
    type Parameters = InstanceParameters;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(p: Self::Parameters) -> Self::Strategy {
        p.validate().unwrap();
        let InstanceParameters {
            num_constraints,
            objective,
            constraint,
            kinds,
        } = p;

        (
            Function::arbitrary_with(objective),
            arbitrary_constraints(num_constraints, constraint),
            Just(kinds),
        )
            .prop_flat_map(|(objective, constraints, kinds)| {
                let mut used_ids = objective.used_decision_variable_ids();
                for c in &constraints {
                    used_ids.extend(c.function().used_decision_variable_ids());
                }

                (
                    Just(objective),
                    Just(constraints),
                    arbitrary_split(used_ids),
                )
                    .prop_flat_map(
                        move |(objective, constraints, (decision_variable_ids, parameter_ids))| {
                            (
                                Just(objective),
                                Just(constraints),
                                arbitrary_decision_variables(decision_variable_ids, kinds.clone()),
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
                                            objective: Some(objective),
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

    fn arbitrary() -> Self::Strategy {
        Self::Parameters::default()
            .smaller()
            .prop_flat_map(Self::arbitrary_with)
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
