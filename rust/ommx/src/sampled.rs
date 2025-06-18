mod approx;
mod parse;

use anyhow::{bail, Result};
use derive_more::{Deref, From};
use fnv::{FnvHashMap, FnvHashSet};
use std::hash::Hash;

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, From, Deref)]
pub struct SampleID(u64);

impl SampleID {
    pub fn into_inner(self) -> u64 {
        self.0
    }
}

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

#[derive(Debug, thiserror::Error)]
#[error("Duplicated sample ID: {id:?}")]
pub struct DuplicatedSampleIDError {
    id: SampleID,
}

#[derive(Debug, thiserror::Error)]
#[error("Unknown sample ID: {id:?}")]
pub struct UnknownSampleIDError {
    id: SampleID,
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

    pub fn iter(&self) -> impl Iterator<Item = (&SampleID, &T)> {
        self.offsets.iter().map(move |(id, offset)| {
            debug_assert!(*offset < self.data.len());
            (id, &self.data[*offset])
        })
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
        self.offsets.get(&sample_id)
            .map(|&offset| {
                debug_assert!(offset < self.data.len());
                &self.data[offset]
            })
            .ok_or(UnknownSampleIDError { id: sample_id })
    }

    /// Get a mutable reference to the value for a specific sample ID
    pub fn get_mut(&mut self, sample_id: SampleID) -> Result<&mut T, UnknownSampleIDError> {
        let sample_id_copy = sample_id; // Copy for error case
        self.offsets.get(&sample_id)
            .map(|&offset| {
                debug_assert!(offset < self.data.len());
                &mut self.data[offset]
            })
            .ok_or(UnknownSampleIDError { id: sample_id_copy })
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
    fn test_sampled_get_mut() {
        let mut sampled = Sampled::new(
            [[SampleID(1), SampleID(2)], [SampleID(5), SampleID(7)]],
            [10, 20],
        )
        .unwrap();

        // Test successful get_mut
        *sampled.get_mut(SampleID(1)).unwrap() = 100;
        assert_eq!(sampled.get(SampleID(1)).unwrap(), &100);
        assert_eq!(sampled.get(SampleID(2)).unwrap(), &100); // Same data

        // Test get_mut with unknown sample ID
        assert!(sampled.get_mut(SampleID(999)).is_err());
    }
}
