use crate::{
    random::FunctionParameters,
    v1::{Constraint, Equality, Function, RemovedConstraint},
};
use proptest::prelude::*;

impl Arbitrary for Constraint {
    type Parameters = <Function as Arbitrary>::Parameters;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(parameters: Self::Parameters) -> Self::Strategy {
        let function = proptest::option::of(Function::arbitrary_with(parameters));
        let equality = prop_oneof![
            Just(Equality::EqualToZero),
            Just(Equality::LessThanOrEqualToZero)
        ];
        (function, equality)
            .prop_map(|(function, equality)| Constraint {
                id: 0, // ID should be changed when creating an instance
                function,
                equality: equality as i32,
                ..Default::default()
            })
            .boxed()
    }

    fn arbitrary() -> Self::Strategy {
        let FunctionParameters {
            num_terms,
            max_degree,
            max_id,
        } = FunctionParameters::default();
        (0..=num_terms, 0..=max_degree, 0..=max_id)
            .prop_flat_map(|(num_terms, max_degree, max_id)| {
                Self::arbitrary_with(FunctionParameters {
                    num_terms,
                    max_degree,
                    max_id,
                })
            })
            .boxed()
    }
}

pub fn arbitrary_constraints(
    num_constraints: usize,
    parameters: <Constraint as Arbitrary>::Parameters,
) -> BoxedStrategy<Vec<Constraint>> {
    let constraints =
        proptest::collection::vec(Constraint::arbitrary_with(parameters), num_constraints);
    let constraint_ids = prop_oneof![
        // continuous case
        Just((0..(num_constraints as u64)).collect::<Vec<u64>>()).prop_shuffle(),
        // discrete case
        Just((0..(3 * num_constraints as u64)).collect::<Vec<u64>>()).prop_shuffle(),
    ];
    (constraints, constraint_ids)
        .prop_map(|(mut c, id)| {
            for (id, c) in id.iter().zip(c.iter_mut()) {
                c.id = *id;
            }
            c
        })
        .boxed()
}

impl Arbitrary for RemovedConstraint {
    type Parameters = <Constraint as Arbitrary>::Parameters;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(parameters: Self::Parameters) -> Self::Strategy {
        (
            Constraint::arbitrary_with(parameters),
            ".{0,3}",
            proptest::collection::hash_map(".{0,3}", ".{0,3}", 0..=2),
        )
            .prop_map(
                |(constraint, removed_reason, removed_reason_parameters)| RemovedConstraint {
                    constraint: Some(constraint),
                    removed_reason,
                    removed_reason_parameters,
                },
            )
            .boxed()
    }

    fn arbitrary() -> Self::Strategy {
        Self::Parameters::default()
            .smaller()
            .prop_flat_map(Self::arbitrary_with)
            .boxed()
    }
}
