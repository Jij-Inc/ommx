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

impl<T> PartialEq for Sampled<T>
where
    T: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        if self.offsets.len() != other.offsets.len() {
            return false;
        }
        for (id, offset) in self.offsets.iter() {
            debug_assert!(*offset < self.data.len());
            let Some(other_offset) = other.offsets.get(id) else {
                return false;
            };
            debug_assert!(*other_offset < other.data.len());
            if self.data[*offset] != other.data[*other_offset] {
                return false;
            }
        }
        true
    }
}

impl<T> Sampled<T> {
    pub fn constants(ids: impl Iterator<Item = SampleID>, value: T) -> Self {
        let map = ids.map(|id| (id, 0)).collect();
        let data = vec![value];
        Self { offsets: map, data }
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
