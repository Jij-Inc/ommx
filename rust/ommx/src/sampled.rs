mod approx;

use anyhow::{bail, Result};
use derive_more::{Deref, From};
use fnv::FnvHashMap;
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

impl<T> Sampled<T> {
    pub fn constants(ids: impl Iterator<Item = SampleID>, value: T) -> Self {
        let map = ids.map(|id| (id, 0)).collect();
        let data = vec![value];
        Self { offsets: map, data }
    }

    pub fn new<'a, Iter, Inner>(ids: Iter, data: impl IntoIterator<Item = T>) -> Result<Self>
    where
        Iter: IntoIterator<Item = Inner>,
        Inner: IntoIterator<Item = SampleID>,
    {
        let data = data.into_iter().collect::<Vec<_>>();
        let mut max_offset = 0;
        let mut offsets = FnvHashMap::default();
        for (offset, inner) in ids.into_iter().enumerate() {
            max_offset = max_offset.max(offset);
            for id in inner {
                if offsets.insert(id, offset).is_some() {
                    bail!("Duplicate sample ID: {:?}", id);
                }
            }
        }
        if data.len() != max_offset + 1 {
            bail!(
                "Data length ({}) does not match the number of samples ({})",
                data.len(),
                max_offset + 1
            );
        }
        Ok(Self { offsets, data })
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
}
