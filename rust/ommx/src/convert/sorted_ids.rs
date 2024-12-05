use proptest::prelude::*;
use std::{collections::BTreeSet, ops::*};

/// A sorted list of decision variable and parameter IDs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortedIds(Vec<u64>);

impl From<Vec<u64>> for SortedIds {
    fn from(ids: Vec<u64>) -> Self {
        Self::new(ids)
    }
}

impl Deref for SortedIds {
    type Target = [u64];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Graded lexicographic order
///
/// - Higher grade comes first
/// - If grades are equal, lexicographic order is used
///
impl Ord for SortedIds {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let a = &self.0;
        let b = &other.0;
        if a.len() != b.len() {
            b.len().cmp(&a.len())
        } else {
            a.cmp(b)
        }
    }
}

impl PartialOrd for SortedIds {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl FromIterator<u64> for SortedIds {
    fn from_iter<I: IntoIterator<Item = u64>>(iter: I) -> Self {
        let ids = iter.into_iter().collect::<Vec<_>>();
        Self::new(ids)
    }
}

impl From<Option<u64>> for SortedIds {
    fn from(id: Option<u64>) -> Self {
        id.into_iter().collect()
    }
}

impl SortedIds {
    pub fn new(ids: Vec<u64>) -> Self {
        let mut ids = ids;
        ids.sort_unstable();
        Self(ids)
    }

    pub fn into_inner(self) -> Vec<u64> {
        self.0
    }

    pub fn empty() -> Self {
        Self(Vec::new())
    }
}

impl Add for SortedIds {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        let mut ids = self.0;
        ids.extend(other.0);
        ids.sort_unstable();
        Self(ids)
    }
}

impl Arbitrary for SortedIds {
    type Parameters = (u32, u64);
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with((max_degree, max_id): Self::Parameters) -> Self::Strategy {
        proptest::collection::vec(0..=max_id, 0..=(max_degree as usize))
            .prop_map(SortedIds::new)
            .boxed()
    }

    fn arbitrary() -> Self::Strategy {
        (0..5_u32, 0..10_u64)
            .prop_flat_map(Self::arbitrary_with)
            .boxed()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryIds(BTreeSet<u64>);

impl Ord for BinaryIds {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let a = &self.0;
        let b = &other.0;
        if a.len() != b.len() {
            b.len().cmp(&a.len())
        } else {
            a.cmp(b)
        }
    }
}

impl PartialOrd for BinaryIds {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl From<SortedIds> for BinaryIds {
    fn from(ids: SortedIds) -> Self {
        Self(ids.0.into_iter().collect())
    }
}

impl Deref for BinaryIds {
    type Target = BTreeSet<u64>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
