use super::Bound;
use crate::VariableID;
use std::collections::{btree_map, BTreeMap};

/// A collection of nonempty intervals indexed by decision-variable ID.
///
/// Entries are ordered by ID. Each entry preserves [`Bound`]'s invariants;
/// this collection does not check whether IDs exist in an instance or whether
/// intervals agree with variable kinds or fixed values. Those checks belong to
/// the instance when applying bounds.
///
/// In an intersection, an absent ID imposes no additional restriction. An empty
/// collection therefore represents no restrictions, not an empty feasible set.
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    crate::logical_memory::LogicalMemoryProfile,
)]
#[serde(transparent)]
pub struct Bounds(BTreeMap<VariableID, Bound>);

impl Bounds {
    /// Create an empty collection of bounds.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of explicitly bounded variable IDs.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether this collection has no entries.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Whether a bound is explicitly stored for `id`.
    pub fn contains_key(&self, id: &VariableID) -> bool {
        self.0.contains_key(id)
    }

    /// Look up a stored bound. An absent ID returns `None`.
    pub fn get(&self, id: &VariableID) -> Option<&Bound> {
        self.0.get(id)
    }

    /// Insert or replace a bound, returning the previous bound if present.
    ///
    /// Use [`Self::intersect_with`] to impose additional restrictions instead
    /// of replacing existing ones.
    pub fn insert(&mut self, id: VariableID, bound: Bound) -> Option<Bound> {
        self.0.insert(id, bound)
    }

    /// Iterate over entries in variable-ID order.
    pub fn iter(&self) -> btree_map::Iter<'_, VariableID, Bound> {
        self.0.iter()
    }

    /// Iterate over the stored variable IDs in order.
    pub fn keys(&self) -> btree_map::Keys<'_, VariableID, Bound> {
        self.0.keys()
    }

    /// Consume this collection and iterate over its variable IDs in order.
    pub fn into_keys(self) -> btree_map::IntoKeys<VariableID, Bound> {
        self.0.into_keys()
    }

    /// Iterate over bounds in variable-ID order.
    pub fn values(&self) -> btree_map::Values<'_, VariableID, Bound> {
        self.0.values()
    }

    /// Intersect bounds for matching IDs, retaining entries present on only one side.
    ///
    /// Returns `None` if any pair of intervals has an empty intersection.
    /// Neither input is changed. No tolerance or variable-kind rounding is applied.
    pub fn intersection(&self, other: &Self) -> Option<Self> {
        let mut intersection = self.clone();
        intersection.intersect_with(other)?;
        Some(intersection)
    }

    /// Impose the restrictions from `other` atomically.
    ///
    /// Matching IDs use [`Bound::intersection`]; IDs present only in `other`
    /// are inserted. Returns `None` and leaves `self` unchanged if any
    /// intersection is empty. No tolerance or variable-kind rounding is applied.
    pub fn intersect_with(&mut self, other: &Self) -> Option<()> {
        let updates = other
            .iter()
            .map(|(&id, &bound)| {
                let intersected = match self.get(&id) {
                    Some(previous) => previous.intersection(&bound)?,
                    None => bound,
                };
                Some((id, intersected))
            })
            .collect::<Option<Vec<_>>>()?;
        self.0.extend(updates);
        Some(())
    }
}

impl From<BTreeMap<VariableID, Bound>> for Bounds {
    fn from(bounds: BTreeMap<VariableID, Bound>) -> Self {
        Self(bounds)
    }
}

impl From<Bounds> for BTreeMap<VariableID, Bound> {
    fn from(bounds: Bounds) -> Self {
        bounds.0
    }
}

impl<const N: usize> From<[(VariableID, Bound); N]> for Bounds {
    fn from(bounds: [(VariableID, Bound); N]) -> Self {
        Self(BTreeMap::from(bounds))
    }
}

impl FromIterator<(VariableID, Bound)> for Bounds {
    fn from_iter<T: IntoIterator<Item = (VariableID, Bound)>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl IntoIterator for Bounds {
    type Item = (VariableID, Bound);
    type IntoIter = btree_map::IntoIter<VariableID, Bound>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a Bounds {
    type Item = (&'a VariableID, &'a Bound);
    type IntoIter = btree_map::Iter<'a, VariableID, Bound>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl std::ops::Index<&VariableID> for Bounds {
    type Output = Bound;

    fn index(&self, id: &VariableID) -> &Self::Output {
        &self.0[id]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intersection_combines_restrictions_and_retains_one_sided_ids() {
        let left = Bounds::from([
            (0.into(), Bound::new(-2.0, 3.0).unwrap()),
            (1.into(), Bound::positive()),
            (3.into(), Bound::new(0.0, 1.0).unwrap()),
        ]);
        let right = Bounds::from([
            (0.into(), Bound::new(-1.0, 4.0).unwrap()),
            (2.into(), Bound::negative()),
            (3.into(), Bound::new(1.0, 2.0).unwrap()),
        ]);
        let expected = Bounds::from([
            (0.into(), Bound::new(-1.0, 3.0).unwrap()),
            (1.into(), Bound::positive()),
            (2.into(), Bound::negative()),
            (3.into(), Bound::new(1.0, 1.0).unwrap()),
        ]);
        assert_eq!(left.intersection(&right), Some(expected.clone()));
        assert_eq!(right.intersection(&left), Some(expected.clone()));
        let mut updated = left;
        assert_eq!(updated.intersect_with(&right), Some(()));
        assert_eq!(updated, expected);
    }

    #[test]
    fn empty_intersection_leaves_all_entries_unchanged() {
        let original = Bounds::from([
            (0.into(), Bound::new(0.0, 3.0).unwrap()),
            (2.into(), Bound::new(0.0, 1.0).unwrap()),
        ]);
        let other = Bounds::from([
            (0.into(), Bound::new(0.0, 2.0).unwrap()),
            (1.into(), Bound::positive()),
            (2.into(), Bound::new(2.0, 3.0).unwrap()),
        ]);
        assert_eq!(original.intersection(&other), None);
        assert_eq!(other.intersection(&original), None);
        let mut updated = original.clone();
        assert_eq!(updated.intersect_with(&other), None);
        assert_eq!(updated, original);
    }

    #[test]
    fn empty_collection_and_unbounded_entries_impose_no_restriction() {
        let bounds = Bounds::from([(0.into(), Bound::new(1.0, 2.0).unwrap())]);
        assert_eq!(bounds.intersection(&Bounds::new()), Some(bounds.clone()));
        assert_eq!(Bounds::new().intersection(&bounds), Some(bounds.clone()));
        assert_eq!(
            bounds.intersection(&Bounds::from([(0.into(), Bound::unbounded())])),
            Some(bounds)
        );
        assert_eq!(
            Bounds::new().intersection(&Bounds::new()),
            Some(Bounds::new())
        );
    }

    #[test]
    fn map_conversions_and_serialization_preserve_entries() {
        let map = BTreeMap::from([(1.into(), Bound::new(-2.0, 3.0).unwrap())]);
        let bounds = Bounds::from(map.clone());
        let json = serde_json::to_string(&bounds).unwrap();
        assert_eq!(json, serde_json::to_string(&map).unwrap());
        assert_eq!(serde_json::from_str::<Bounds>(&json).unwrap(), bounds);
        assert_eq!(
            bounds.iter().map(|(&id, &b)| (id, b)).collect::<Bounds>(),
            bounds
        );
        assert_eq!(BTreeMap::from(bounds), map);
    }
}
