use crate::{
    random::{arbitrary_constraints, arbitrary_decision_variables, FunctionParameters},
    v1::{
        decision_variable::Kind,
        instance::{Description, Sense},
        Function, Instance,
    },
};
use proptest::prelude::*;

impl Instance {
    pub fn arbitrary_lp() -> BoxedStrategy<Self> {
        (0..10_usize, 0..10_usize, 0..=1_u32, 0..10_u64)
            .prop_flat_map(Self::arbitrary_with)
            .boxed()
    }

    pub fn arbitrary_binary() -> BoxedStrategy<Self> {
        (0..10_usize, 0..10_usize, 0..=4_u32, 0..10_u64)
            .prop_flat_map(|(num_constraints, num_terms, max_degree, max_id)| {
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
        (0..10_usize, 0..=4_u32, 0..10_u64)
            .prop_flat_map(|(num_terms, max_degree, max_id)| {
                arbitrary_instance(0, num_terms, max_degree, max_id, Just(Kind::Binary))
            })
            .boxed()
    }

    pub fn arbitrary_quadratic_binary_unconstrained() -> BoxedStrategy<Self> {
        (0..10_usize, 0..=2_u32, 0..10_u64)
            .prop_flat_map(|(num_terms, max_degree, max_id)| {
                arbitrary_instance(0, num_terms, max_degree, max_id, Just(Kind::Binary))
            })
            .boxed()
    }
}

impl Arbitrary for Instance {
    type Parameters = (usize, usize, u32, u64);
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(
        (num_constraints, num_terms, max_degree, max_id): Self::Parameters,
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
