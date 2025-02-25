use crate::{
    random::{arbitrary_constraints, arbitrary_decision_variables, FunctionParameters},
    v1::{
        decision_variable::Kind,
        instance::{Description, Sense},
        Function, Instance,
    },
};
use proptest::prelude::*;

use super::num_terms_and_max_id;

impl Instance {
    /// Arbitrary LP problem, i.e. linear objective and constraints with continuous decision variables.
    pub fn arbitrary_lp() -> BoxedStrategy<Self> {
        let InstanceParameters {
            num_constraints,
            num_terms,
            max_id,
            ..
        } = Default::default();
        (0..=num_constraints, num_terms_and_max_id(num_terms, max_id))
            .prop_flat_map(|(num_constraints, (num_terms, max_id))| {
                arbitrary_instance(
                    num_constraints,
                    num_terms,
                    1,
                    max_id,
                    Just(Kind::Continuous),
                )
            })
            .boxed()
    }

    pub fn arbitrary_binary() -> BoxedStrategy<Self> {
        (0..10_usize, 0..=4_u32, num_terms_and_max_id(5, 10))
            .prop_flat_map(|(num_constraints, max_degree, (num_terms, max_id))| {
                arbitrary_instance(
                    num_constraints,
                    num_terms,
                    max_degree,
                    max_id,
                    Just(Kind::Binary),
                )
            })
            .boxed()
    }

    pub fn arbitrary_binary_unconstrained() -> BoxedStrategy<Self> {
        (0..=4_u32, num_terms_and_max_id(5, 10))
            .prop_flat_map(|(max_degree, (num_terms, max_id))| {
                arbitrary_instance(0, num_terms, max_degree, max_id, Just(Kind::Binary))
            })
            .boxed()
    }

    pub fn arbitrary_quadratic_binary_unconstrained() -> BoxedStrategy<Self> {
        (0..=2_u32, num_terms_and_max_id(5, 10))
            .prop_flat_map(|(max_degree, (num_terms, max_id))| {
                arbitrary_instance(0, num_terms, max_degree, max_id, Just(Kind::Binary))
            })
            .boxed()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InstanceParameters {
    pub num_constraints: usize,
    pub num_terms: usize,
    pub max_degree: u32,
    pub max_id: u64,
}

impl Default for InstanceParameters {
    fn default() -> Self {
        Self {
            num_constraints: 5,
            num_terms: 5,
            max_degree: 3,
            max_id: 10,
        }
    }
}

impl Arbitrary for Instance {
    type Parameters = InstanceParameters;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(
        InstanceParameters {
            num_constraints,
            num_terms,
            max_degree,
            max_id,
        }: Self::Parameters,
    ) -> Self::Strategy {
        arbitrary_instance(
            num_constraints,
            num_terms,
            max_degree,
            max_id,
            Kind::arbitrary(),
        )
    }

    fn arbitrary() -> Self::Strategy {
        let InstanceParameters {
            num_constraints,
            num_terms,
            max_degree,
            max_id,
        } = Default::default();
        (
            0..=num_constraints,
            0..=max_degree,
            num_terms_and_max_id(num_terms, max_id),
        )
            .prop_flat_map(|(num_constraints, max_degree, (num_terms, max_id))| {
                arbitrary_instance(
                    num_constraints,
                    num_terms,
                    max_degree,
                    max_id,
                    Kind::arbitrary(),
                )
            })
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
            proptest::option::of(".{0,3}"),
            proptest::option::of(".{0,3}"),
            prop_oneof![Just(Vec::new()), proptest::collection::vec(".*", 1..3)],
            proptest::option::of(".{0,3}"),
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

fn arbitrary_instance(
    num_constraints: usize,
    num_terms: usize,
    max_degree: u32,
    max_id: u64,
    kind_strategy: impl Strategy<Value = Kind> + 'static + Clone,
) -> BoxedStrategy<Instance> {
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
        .prop_flat_map(move |(objective, constraints)| {
            let mut used_ids = objective
                .as_ref()
                .map(|f| f.used_decision_variable_ids())
                .unwrap_or_default();
            for c in &constraints {
                used_ids.extend(c.function().used_decision_variable_ids());
            }
            let relaxed = if constraints.is_empty() {
                Just(Vec::new()).boxed()
            } else {
                let constraint_ids = constraints.iter().map(|c| c.id).collect::<Vec<_>>();
                proptest::sample::subsequence(constraint_ids, 0..=constraints.len()).boxed()
            };
            (
                Just(objective),
                Just(constraints),
                arbitrary_decision_variables(used_ids, kind_strategy.clone()),
                Option::<Description>::arbitrary(),
                Sense::arbitrary(),
                relaxed,
                ".{0,3}",
                proptest::collection::hash_map(".{0,3}", ".{0,3}", 0..=2),
            )
                .prop_map(
                    |(
                        objective,
                        constraints,
                        decision_variables,
                        description,
                        sense,
                        relaxed,
                        removed_reason,
                        removed_parameters,
                    )| {
                        let mut instance = Instance {
                            objective,
                            constraints,
                            decision_variables,
                            description,
                            sense: sense as i32,
                            ..Default::default()
                        };
                        for i in relaxed {
                            instance
                                .relax_constraint(
                                    i,
                                    removed_reason.clone(),
                                    removed_parameters.clone(),
                                )
                                .unwrap();
                        }
                        instance
                    },
                )
        })
        .boxed()
}
