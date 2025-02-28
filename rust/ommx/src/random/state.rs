use crate::{random::arbitrary_coefficient, v1::State};
use proptest::prelude::*;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StateParameters {
    pub size: usize,
    pub max_id: u64,
}

impl Default for StateParameters {
    fn default() -> Self {
        Self {
            size: 5,
            max_id: 10,
        }
    }
}

impl Arbitrary for State {
    type Parameters = StateParameters;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(StateParameters { size, max_id }: Self::Parameters) -> Self::Strategy {
        proptest::collection::hash_map(0..=max_id, arbitrary_coefficient(), 0..=size)
            .prop_map(Self::from)
            .boxed()
    }

    fn arbitrary() -> Self::Strategy {
        let StateParameters { size, max_id } = StateParameters::default();
        (0..=size, 0..=max_id)
            .prop_flat_map(|(size, max_id)| {
                proptest::collection::hash_map(0..=max_id, arbitrary_coefficient(), 0..=size)
                    .prop_map(move |entries| State { entries })
            })
            .boxed()
    }
}

pub fn arbitrary_state(ids: BTreeSet<u64>) -> BoxedStrategy<State> {
    (
        proptest::collection::vec(arbitrary_coefficient(), ids.len()),
        Just(ids),
    )
        .prop_map(|(coefficients, ids)| {
            let entries = ids.into_iter().zip(coefficients).collect();
            State { entries }
        })
        .boxed()
}
