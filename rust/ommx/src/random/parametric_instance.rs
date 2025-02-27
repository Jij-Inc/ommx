use crate::{
    random::{
        arbitrary_constraints, arbitrary_decision_variables, arbitrary_parameters,
        FunctionParameters,
    },
    v1::{
        decision_variable::Kind,
        instance::{Description, Sense},
        Function, ParametricInstance,
    },
};
use proptest::prelude::*;
use std::collections::BTreeSet;

pub struct ParametricInstanceParameters {
    pub num_constraints: usize,
    pub num_terms: usize,
    pub max_degree: u32,
    pub max_id: u64,
}

impl Default for ParametricInstanceParameters {
    fn default() -> Self {
        Self {
            num_constraints: 5,
            num_terms: 5,
            max_degree: 2,
            max_id: 10,
        }
    }
}

impl Arbitrary for ParametricInstance {
    type Parameters = ParametricInstanceParameters;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(
        ParametricInstanceParameters {
            num_constraints,
            num_terms,
            max_degree,
            max_id,
        }: Self::Parameters,
    ) -> Self::Strategy {
        (
            proptest::option::of(Function::arbitrary_with(FunctionParameters {
                num_terms,
                max_degree,
                max_id,
            })),
            arbitrary_constraints(
                num_constraints,
                FunctionParameters {
                    num_terms,
                    max_degree,
                    max_id,
                },
            ),
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
                                    Kind::possibles(),
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

    fn arbitrary() -> Self::Strategy {
        todo!()
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
