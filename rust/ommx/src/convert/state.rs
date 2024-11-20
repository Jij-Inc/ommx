use crate::v1::State;
use proptest::prelude::*;
use std::collections::HashMap;

impl From<HashMap<u64, f64>> for State {
    fn from(value: HashMap<u64, f64>) -> Self {
        Self { entries: value }
    }
}

impl FromIterator<(u64, f64)> for State {
    fn from_iter<T: IntoIterator<Item = (u64, f64)>>(iter: T) -> Self {
        Self {
            entries: iter.into_iter().collect(),
        }
    }
}

impl IntoIterator for State {
    type Item = (u64, f64);
    type IntoIter = std::collections::hash_map::IntoIter<u64, f64>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

impl Arbitrary for State {
    type Parameters = (usize, u64);
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with((size, max_id): Self::Parameters) -> Self::Strategy {
        proptest::collection::hash_map(0..=max_id, super::arbitrary_coefficient(), 0..=size)
            .prop_map(Self::from)
            .boxed()
    }

    fn arbitrary() -> Self::Strategy {
        (0..20_usize, 0..20_u64)
            .prop_flat_map(Self::arbitrary_with)
            .boxed()
    }
}
