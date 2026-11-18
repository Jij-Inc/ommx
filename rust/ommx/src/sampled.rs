mod approx;
mod parse;

use anyhow::{bail, Result};
use derive_more::{Deref, From};
use fnv::{FnvHashMap, FnvHashSet};
use std::{collections::BTreeSet, hash::Hash};

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, From, Deref)]
pub struct SampleID(u64);

impl SampleID {
    pub fn into_inner(self) -> u64 {
        self.0
    }
}

pub type SampleIDSet = BTreeSet<SampleID>;

/// Values indexed by sample ID, optionally sharing one stored value across IDs.
///
/// Each ID identifies exactly one value, and every stored value has at least one
/// ID. An empty collection stores no values. Constructors and [`Self::append`]
/// preserve these invariants; validation of a state against a model belongs to
/// the model's evaluation operation.
#[derive(Debug, Clone)]
pub struct Sampled<T> {
    offsets: FnvHashMap<SampleID, usize>,
    data: Vec<T>,
}

impl<T> Default for Sampled<T> {
    fn default() -> Self {
        Self {
            offsets: FnvHashMap::default(),
            data: Vec::new(),
        }
    }
}

impl<T> From<T> for Sampled<T> {
    fn from(value: T) -> Self {
        let mut offsets = FnvHashMap::default();
        offsets.insert(SampleID(0), 0);
        Self {
            offsets,
            data: vec![value],
        }
    }
}

impl<T> From<(SampleID, T)> for Sampled<T> {
    fn from((id, value): (SampleID, T)) -> Self {
        let mut offsets = FnvHashMap::default();
        offsets.insert(id, 0);
        Self {
            offsets,
            data: vec![value],
        }
    }
}

/// A sample ID already present in a collection or repeated in constructor or append input.
///
/// Use [`Self::id`] to identify the conflicting ID before correcting the input
/// and retrying the atomic operation.
///
/// ```
/// let mut sampled = ommx::Sampled::from((ommx::SampleID::from(0), "existing"));
/// let error = sampled
///     .append([ommx::SampleID::from(0)], "replacement")
///     .unwrap_err();
/// let error = error.downcast_ref::<ommx::DuplicatedSampleIDError>().unwrap();
/// assert_eq!(error.id(), ommx::SampleID::from(0));
/// ```
#[derive(Debug, thiserror::Error)]
#[error("Duplicated sample ID: {id:?}")]
pub struct DuplicatedSampleIDError {
    id: SampleID,
}

impl DuplicatedSampleIDError {
    /// Return the sample ID that caused the duplicate-ID failure.
    pub fn id(&self) -> SampleID {
        self.id
    }
}

/// An appended value has no sample ID identifying it.
///
/// Supply at least one sample ID before retrying [`Sampled::append`] or
/// [`Sampled::new`]. An empty collection is valid; represent it with
/// [`Sampled::default`] rather than an entry with no IDs.
#[derive(Debug, thiserror::Error)]
#[error("sample IDs must contain at least one ID")]
#[non_exhaustive]
pub struct EmptySampleIDs;

impl<T> Sampled<T> {
    /// Associate a constant value with the supplied IDs, or return an empty
    /// collection when there are no IDs.
    pub fn constants(ids: impl Iterator<Item = SampleID>, value: T) -> Self {
        let map: FnvHashMap<_, _> = ids.map(|id| (id, 0)).collect();
        if map.is_empty() {
            return Self::default();
        }
        let data = vec![value];
        Self { offsets: map, data }
    }

    /// Append one value under every supplied sample ID.
    ///
    /// # Errors
    ///
    /// The error chain contains [`EmptySampleIDs`] if `ids` is empty, or
    /// [`DuplicatedSampleIDError`] if an ID already exists in this collection
    /// or occurs more than once in `ids`. All IDs are validated before mutation,
    /// so the collection is unchanged when this method fails.
    pub fn append(
        &mut self,
        ids: impl IntoIterator<Item = SampleID>,
        value: T,
    ) -> crate::Result<()> {
        let mut new_ids = Vec::new();
        let mut seen = FnvHashSet::default();
        for id in ids {
            if self.offsets.contains_key(&id) || !seen.insert(id) {
                return Err(DuplicatedSampleIDError { id }.into());
            }
            new_ids.push(id);
        }
        if new_ids.is_empty() {
            return Err(EmptySampleIDs.into());
        }

        let offset = self.data.len();
        self.data.push(value);
        for id in new_ids {
            self.offsets.insert(id, offset);
        }
        Ok(())
    }

    pub fn new<Iter, Inner>(ids: Iter, data: impl IntoIterator<Item = T>) -> Result<Self>
    where
        Iter: IntoIterator<Item = Inner>,
        Inner: IntoIterator<Item = SampleID>,
    {
        let mut out = Self::default();
        let mut ids_iter = ids.into_iter();
        let mut data_iter = data.into_iter();
        loop {
            match (ids_iter.next(), data_iter.next()) {
                (Some(ids), Some(data)) => out.append(ids, data)?,
                (None, None) => break,
                (Some(_), None) => bail!("Data length mismatch"),
                (None, Some(_)) => bail!("Sample IDs length mismatch"),
            }
        }
        Ok(out)
    }

    /// Construct samples without sharing storage between different IDs.
    ///
    /// # Errors
    ///
    /// Returns [`DuplicatedSampleIDError`] in the error chain if an ID occurs
    /// more than once, even when the associated values are equal.
    pub fn new_no_dedup(iter: impl Iterator<Item = (SampleID, T)>) -> crate::Result<Self> {
        let mut offsets = FnvHashMap::default();
        let mut data = Vec::new();
        for (id, value) in iter {
            match offsets.entry(id) {
                std::collections::hash_map::Entry::Occupied(_) => {
                    return Err(DuplicatedSampleIDError { id }.into());
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(data.len());
                    data.push(value);
                }
            }
        }
        Ok(Self { offsets, data })
    }

    /// Construct samples, sharing storage between equal values.
    ///
    /// # Errors
    ///
    /// Returns [`DuplicatedSampleIDError`] in the error chain if an ID occurs
    /// more than once, even when the associated values are equal. Equal values
    /// under different IDs are valid and share storage.
    pub fn new_dedup<I>(iter: I) -> crate::Result<Self>
    where
        I: Iterator<Item = (SampleID, T)>,
        T: Hash + Eq + Clone,
    {
        let mut offsets = FnvHashMap::default();
        let mut data = Vec::new();
        let mut value_to_offset: FnvHashMap<T, usize> = FnvHashMap::default();

        for (id, value) in iter {
            let entry = match offsets.entry(id) {
                std::collections::hash_map::Entry::Occupied(_) => {
                    return Err(DuplicatedSampleIDError { id }.into());
                }
                std::collections::hash_map::Entry::Vacant(entry) => entry,
            };
            // Check if we already have this value using HashMap lookup (O(1))
            let offset = match value_to_offset.get(&value) {
                Some(&existing_offset) => {
                    // Reuse existing data
                    existing_offset
                }
                None => {
                    // Add new data
                    let new_offset = data.len();
                    value_to_offset.insert(value.clone(), new_offset);
                    data.push(value);
                    new_offset
                }
            };
            entry.insert(offset);
        }

        Ok(Self { offsets, data })
    }

    pub fn iter(&self) -> impl Iterator<Item = (&SampleID, &T)> {
        self.offsets.iter().map(move |(id, offset)| {
            debug_assert!(*offset < self.data.len());
            (id, &self.data[*offset])
        })
    }

    /// Mutable iterator over the *unique* stored values.
    ///
    /// Each distinct `T` is yielded once, even when multiple [`SampleID`]s
    /// point at it. Mutating the value therefore mutates it for every sample
    /// that references it.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.data.iter_mut()
    }

    pub fn ids(&self) -> SampleIDSet {
        self.offsets.keys().copied().collect()
    }

    pub fn has_same_ids(&self, ids: &SampleIDSet) -> bool {
        if self.offsets.len() != ids.len() {
            return false;
        }
        // Check that all IDs in the set are present in our offsets
        ids.iter().all(|id| self.offsets.contains_key(id))
    }

    pub fn map<U, F: FnMut(T) -> U>(self, f: F) -> Sampled<U> {
        Sampled {
            offsets: self.offsets,
            data: self.data.into_iter().map(f).collect(),
        }
    }

    /// Non-consuming, fallible variant of [`Self::map`].
    ///
    /// Applies `f` to each unique stored value once; sample-id grouping is
    /// preserved. Useful when evaluating per-sample-state like in
    /// `Evaluate::evaluate_samples`.
    pub fn try_map_ref<U, E>(
        &self,
        mut f: impl FnMut(&T) -> std::result::Result<U, E>,
    ) -> std::result::Result<Sampled<U>, E> {
        let data = self
            .data
            .iter()
            .map(&mut f)
            .collect::<std::result::Result<Vec<_>, E>>()?;
        Ok(Sampled {
            offsets: self.offsets.clone(),
            data,
        })
    }

    pub fn num_samples(&self) -> usize {
        self.offsets.len()
    }

    /// Get a reference to the value for a specific sample ID.
    ///
    /// Returns [`None`] if the sample ID is not known to this [`Sampled`].
    pub fn get(&self, sample_id: SampleID) -> Option<&T> {
        self.offsets.get(&sample_id).map(|&offset| {
            debug_assert!(offset < self.data.len());
            &self.data[offset]
        })
    }

    /// Gather up the sample ID for each sample.
    pub fn chunk(self) -> Vec<(T, FnvHashSet<SampleID>)> {
        let mut out = self
            .data
            .into_iter()
            .map(|data| (data, FnvHashSet::default()))
            .collect::<Vec<_>>();
        for (id, offset) in &self.offsets {
            out[*offset].1.insert(*id);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sampled() {
        let sampled = Sampled::new(
            [[SampleID(1), SampleID(2)], [SampleID(5), SampleID(7)]],
            [1, 2],
        )
        .unwrap();
        assert_eq!(sampled.num_samples(), 4);
        assert_eq!(
            sampled.iter().collect::<Vec<_>>(),
            vec![
                (&SampleID(5), &2),
                (&SampleID(7), &2),
                (&SampleID(1), &1),
                (&SampleID(2), &1),
            ]
        );

        // Size mismatch tests
        assert!(Sampled::new(
            [[SampleID(1), SampleID(2)], [SampleID(5), SampleID(7)]],
            [1, 2, 3],
        )
        .is_err());
        assert!(Sampled::new(
            [[SampleID(1), SampleID(2)], [SampleID(5), SampleID(7)]],
            [1],
        )
        .is_err());
    }

    #[test]
    fn test_sampled_get() {
        let sampled = Sampled::new(
            [[SampleID(1), SampleID(2)], [SampleID(5), SampleID(7)]],
            [10, 20],
        )
        .unwrap();

        // Test successful get
        assert_eq!(sampled.get(SampleID(1)).unwrap(), &10);
        assert_eq!(sampled.get(SampleID(2)).unwrap(), &10);
        assert_eq!(sampled.get(SampleID(5)).unwrap(), &20);
        assert_eq!(sampled.get(SampleID(7)).unwrap(), &20);

        // Test get with unknown sample ID
        assert!(sampled.get(SampleID(999)).is_none());
    }

    #[test]
    fn test_new_dedup() {
        let sampled = Sampled::new_dedup(
            [
                (SampleID(1), 10),
                (SampleID(2), 20),
                (SampleID(3), 10), // Same value as SampleID(1)
                (SampleID(4), 30),
                (SampleID(5), 20), // Same value as SampleID(2)
            ]
            .into_iter(),
        )
        .unwrap();

        // Should have 5 samples but only 3 data entries (deduplication occurred)
        assert_eq!(sampled.num_samples(), 5);
        assert_eq!(sampled.data.len(), 3); // Only 3 data entries stored due to deduplication

        // Test that same values point to the same data
        assert_eq!(sampled.get(SampleID(1)).unwrap(), &10);
        assert_eq!(sampled.get(SampleID(3)).unwrap(), &10); // Same value
        assert_eq!(sampled.get(SampleID(2)).unwrap(), &20);
        assert_eq!(sampled.get(SampleID(5)).unwrap(), &20); // Same value
        assert_eq!(sampled.get(SampleID(4)).unwrap(), &30);

        // Verify that samples with same values share the same offset
        let offset_1 = sampled.offsets[&SampleID(1)];
        let offset_3 = sampled.offsets[&SampleID(3)];
        assert_eq!(offset_1, offset_3); // Should point to same data

        let offset_2 = sampled.offsets[&SampleID(2)];
        let offset_5 = sampled.offsets[&SampleID(5)];
        assert_eq!(offset_2, offset_5); // Should point to same data
    }

    #[test]
    fn test_iter_mut_shared_value_propagation() {
        // Two sample IDs share one stored value via append([a, b], value).
        // Mutating through iter_mut must be visible from both IDs, and the
        // offsets table must not be disturbed.
        let mut sampled = Sampled::<Vec<i32>>::default();
        sampled
            .append([SampleID(1), SampleID(2)], vec![10])
            .unwrap();
        sampled.append([SampleID(3)], vec![20]).unwrap();

        // Distinct values only: iter_mut yields one reference per stored entry,
        // not per sample id.
        assert_eq!(sampled.iter_mut().count(), 2);

        for v in sampled.iter_mut() {
            v.push(99);
        }

        // Both shared IDs see the mutation through the same storage slot.
        assert_eq!(sampled.get(SampleID(1)).unwrap(), &vec![10, 99]);
        assert_eq!(sampled.get(SampleID(2)).unwrap(), &vec![10, 99]);
        assert_eq!(sampled.get(SampleID(3)).unwrap(), &vec![20, 99]);

        // Offsets remain stable: shared IDs still point at the same slot.
        assert_eq!(sampled.offsets[&SampleID(1)], sampled.offsets[&SampleID(2)]);
        assert_ne!(sampled.offsets[&SampleID(1)], sampled.offsets[&SampleID(3)]);
        assert_eq!(sampled.num_samples(), 3);
    }

    #[test]
    fn constructors_reject_repeated_ids_without_consuming_remaining_input() {
        for repeated_value in [10, 20] {
            let pairs = [
                (SampleID(7), 10),
                (SampleID(8), 10),
                (SampleID(7), repeated_value),
            ];
            for dedup in [false, true] {
                let iter = pairs.into_iter().chain(std::iter::once_with(|| {
                    panic!("input after the duplicate must not be consumed")
                }));
                let error = if dedup {
                    Sampled::new_dedup(iter)
                } else {
                    Sampled::new_no_dedup(iter)
                }
                .unwrap_err();
                assert_eq!(
                    error
                        .downcast_ref::<DuplicatedSampleIDError>()
                        .unwrap()
                        .id(),
                    SampleID(7),
                );
            }
        }
    }

    #[test]
    fn empty_id_append_is_rejected_before_mutation() {
        for mut sampled in [Sampled::default(), Sampled::from((SampleID(7), 10))] {
            let before_ids = sampled.ids();
            let before_data = sampled.data.clone();
            for _ in 0..2 {
                let error = sampled.append([], 20).unwrap_err();
                assert!(error.is::<EmptySampleIDs>());
                assert_eq!(sampled.ids(), before_ids);
                assert_eq!(sampled.data, before_data);
            }
            sampled.append([SampleID(8), SampleID(9)], 30).unwrap();
            assert_eq!(sampled.get(SampleID(8)), Some(&30));
            assert_eq!(sampled.get(SampleID(9)), Some(&30));
        }
    }

    #[test]
    fn new_rejects_empty_id_groups_but_accepts_no_groups() {
        let error = Sampled::new([vec![SampleID(0)], vec![]], [10, 20]).unwrap_err();
        assert!(error.is::<EmptySampleIDs>());
        let empty = Sampled::<i32>::new(Vec::<Vec<SampleID>>::new(), []).unwrap();
        assert!(empty.data.is_empty());
        assert!(empty.ids().is_empty());
    }

    #[test]
    fn empty_constants_do_not_store_unaddressable_values() {
        let empty = Sampled::constants(std::iter::empty(), 10);
        assert_eq!(empty.num_samples(), 0);
        assert!(empty.chunk().is_empty());
        let shared = Sampled::constants([SampleID(1), SampleID(2)].into_iter(), 10);
        assert_eq!(shared.num_samples(), 2);
        assert_eq!(shared.chunk().len(), 1);
    }

    #[test]
    fn append_is_atomic_when_an_existing_id_follows_a_fresh_id() {
        let mut sampled = Sampled::from((SampleID(0), 10));

        let error = sampled.append([SampleID(1), SampleID(0)], 20).unwrap_err();

        assert_eq!(
            error
                .downcast_ref::<DuplicatedSampleIDError>()
                .unwrap()
                .id(),
            SampleID(0)
        );
        assert_eq!(sampled.ids(), BTreeSet::from([SampleID(0)]));
        assert_eq!(sampled.get(SampleID(0)), Some(&10));
        assert_eq!(sampled.get(SampleID(1)), None);
        assert_eq!(sampled.data, vec![10]);
    }

    #[test]
    fn append_is_atomic_for_duplicate_ids_in_one_input() {
        let mut sampled = Sampled::from((SampleID(0), 10));

        let error = sampled.append([SampleID(1), SampleID(1)], 20).unwrap_err();

        assert_eq!(
            error
                .downcast_ref::<DuplicatedSampleIDError>()
                .unwrap()
                .id(),
            SampleID(1)
        );
        assert_eq!(sampled.ids(), BTreeSet::from([SampleID(0)]));
        assert_eq!(sampled.get(SampleID(0)), Some(&10));
        assert_eq!(sampled.get(SampleID(1)), None);
        assert_eq!(sampled.data, vec![10]);
    }

    #[test]
    fn test_try_map_ref_preserves_offsets_and_propagates_errors() {
        let sampled = Sampled::new(
            [[SampleID(1), SampleID(2)], [SampleID(5), SampleID(7)]],
            [10, 20],
        )
        .unwrap();

        // Happy path: mapping preserves the ID → offset grouping exactly, so
        // IDs that shared storage before still share the mapped value.
        let mapped = sampled.try_map_ref(|v| anyhow::Ok(*v + 1)).unwrap();
        assert_eq!(mapped.num_samples(), 4);
        assert_eq!(mapped.get(SampleID(1)).unwrap(), &11);
        assert_eq!(mapped.get(SampleID(2)).unwrap(), &11);
        assert_eq!(mapped.get(SampleID(5)).unwrap(), &21);
        assert_eq!(mapped.get(SampleID(7)).unwrap(), &21);
        assert_eq!(mapped.offsets[&SampleID(1)], mapped.offsets[&SampleID(2)]);
        assert_eq!(mapped.offsets[&SampleID(5)], mapped.offsets[&SampleID(7)]);

        // Errors short-circuit: a failure on any stored value surfaces to the
        // caller instead of producing a partial Sampled<U>.
        let err = sampled
            .try_map_ref(|v| -> Result<i32> {
                if *v == 20 {
                    anyhow::bail!("boom");
                }
                Ok(*v)
            })
            .unwrap_err();
        assert_eq!(err.to_string(), "boom");
    }

    #[test]
    fn test_new_no_dedup_vs_new_dedup() {
        let data = [
            (SampleID(1), 10),
            (SampleID(2), 20),
            (SampleID(3), 10), // Duplicate value
            (SampleID(4), 20), // Duplicate value
        ];

        let no_dedup = Sampled::new_no_dedup(data.iter().copied()).unwrap();
        let dedup = Sampled::new_dedup(data.iter().copied()).unwrap();

        // Both should have the same number of samples
        assert_eq!(no_dedup.num_samples(), 4);
        assert_eq!(dedup.num_samples(), 4);

        // But different number of data entries
        assert_eq!(no_dedup.data.len(), 4); // No deduplication - each sample has its own data entry
        assert_eq!(dedup.data.len(), 2); // Deduplication applied - duplicate values share data entries

        // Values should be the same
        for (id, _) in data {
            assert_eq!(no_dedup.get(id).unwrap(), dedup.get(id).unwrap());
        }
    }
}
