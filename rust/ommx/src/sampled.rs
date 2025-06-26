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

#[derive(Debug, thiserror::Error)]
#[error("Duplicated sample ID: {id:?}")]
pub struct DuplicatedSampleIDError {
    id: SampleID,
}

#[derive(Debug, thiserror::Error)]
#[error("Unknown sample ID: {id:?}")]
pub struct UnknownSampleIDError {
    pub id: SampleID,
}

impl<T> Sampled<T> {
    pub fn constants(ids: impl Iterator<Item = SampleID>, value: T) -> Self {
        let map = ids.map(|id| (id, 0)).collect();
        let data = vec![value];
        Self { offsets: map, data }
    }

    pub fn append(
        &mut self,
        ids: impl IntoIterator<Item = SampleID>,
        value: T,
    ) -> std::result::Result<(), DuplicatedSampleIDError> {
        let offset = self.data.len();
        self.data.push(value);
        for id in ids {
            if self.offsets.insert(id, offset).is_some() {
                return Err(DuplicatedSampleIDError { id });
            }
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

    pub fn new_no_dedup(iter: impl Iterator<Item = (SampleID, T)>) -> Self {
        let mut offsets = FnvHashMap::default();
        let mut data = Vec::new();
        for (n, (id, value)) in iter.enumerate() {
            offsets.insert(id, n);
            data.push(value);
        }
        Self { offsets, data }
    }

    pub fn new_dedup<I>(iter: I) -> Self
    where
        I: Iterator<Item = (SampleID, T)>,
        T: Hash + Eq + Clone,
    {
        let mut offsets = FnvHashMap::default();
        let mut data = Vec::new();
        let mut value_to_offset: FnvHashMap<T, usize> = FnvHashMap::default();

        for (id, value) in iter {
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
            offsets.insert(id, offset);
        }

        Self { offsets, data }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&SampleID, &T)> {
        self.offsets.iter().map(move |(id, offset)| {
            debug_assert!(*offset < self.data.len());
            (id, &self.data[*offset])
        })
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

    pub fn num_samples(&self) -> usize {
        self.offsets.len()
    }

    /// Get a reference to the value for a specific sample ID
    pub fn get(&self, sample_id: SampleID) -> Result<&T, UnknownSampleIDError> {
        self.offsets
            .get(&sample_id)
            .map(|&offset| {
                debug_assert!(offset < self.data.len());
                &self.data[offset]
            })
            .ok_or(UnknownSampleIDError { id: sample_id })
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
        assert!(sampled.get(SampleID(999)).is_err());
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
        );

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
    fn test_new_no_dedup_vs_new_dedup() {
        let data = [
            (SampleID(1), 10),
            (SampleID(2), 20),
            (SampleID(3), 10), // Duplicate value
            (SampleID(4), 20), // Duplicate value
        ];

        let no_dedup = Sampled::new_no_dedup(data.iter().copied());
        let dedup = Sampled::new_dedup(data.iter().copied());

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
