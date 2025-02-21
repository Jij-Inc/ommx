use crate::v1::Parameter;
use proptest::prelude::*;
use std::collections::{BTreeSet, HashMap};

impl Arbitrary for Parameter {
    type Parameters = u64;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(max_id: Self::Parameters) -> Self::Strategy {
        let subscripts = prop_oneof![
            Just(Vec::<i64>::new()),
            proptest::collection::vec(-(max_id as i64)..=(max_id as i64), 1..=3),
        ];
        let parameters = prop_oneof![
            Just(HashMap::<String, String>::new()),
            proptest::collection::hash_map(".{0,3}", ".{0,3}", 1..=3),
        ];
        (
            0..=max_id,
            proptest::option::of(".{0,3}"),
            subscripts,
            parameters,
            proptest::option::of(".{0,3}"),
        )
            .prop_map(
                |(id, name, subscripts, parameters, description)| Parameter {
                    id,
                    name,
                    subscripts,
                    parameters,
                    description,
                },
            )
            .boxed()
    }
}

pub fn arbitrary_parameters(ids: BTreeSet<u64>) -> BoxedStrategy<Vec<Parameter>> {
    (
        proptest::collection::vec(Parameter::arbitrary(), ids.len()),
        Just(ids),
    )
        .prop_map(|(mut dvs, used_ids)| {
            for (dv, id) in dvs.iter_mut().zip(used_ids.iter()) {
                dv.id = *id;
            }
            dvs
        })
        .boxed()
}
