use derive_more::{Deref, From};
use fnv::FnvHashSet;
use std::hash::Hash;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, From, Deref)]
pub struct SampleID(u64);

impl SampleID {
    pub fn into_inner(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sampled<T>(Vec<(FnvHashSet<SampleID>, T)>);

impl<T> std::ops::Deref for Sampled<T> {
    type Target = [(FnvHashSet<SampleID>, T)];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> Sampled<T> {
    pub fn iter(&self) -> impl Iterator<Item = (&SampleID, &T)> {
        self.0
            .iter()
            .flat_map(|(ids, t)| ids.iter().map(move |id| (id, t)))
    }

    pub fn map<U, F: FnMut(T) -> U>(self, mut f: F) -> Sampled<U> {
        Sampled(self.0.into_iter().map(|(ids, t)| (ids, f(t))).collect())
    }
}
