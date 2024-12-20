use crate::v1::{SampledValues, Samples, State};

impl FromIterator<(u64, f64)> for SampledValues {
    fn from_iter<I: IntoIterator<Item = (u64, f64)>>(iter: I) -> Self {
        Self {
            values: iter.into_iter().collect(),
        }
    }
}

impl IntoIterator for SampledValues {
    type Item = (u64, f64);
    type IntoIter = std::collections::hash_map::IntoIter<u64, f64>;

    fn into_iter(self) -> Self::IntoIter {
        self.values.into_iter()
    }
}

impl SampledValues {
    pub fn iter(&self) -> impl Iterator<Item = (&u64, &f64)> {
        self.values.iter()
    }
}

impl Samples {
    pub fn iter(&self) -> impl Iterator<Item = (&u64, &State)> {
        self.states.iter()
    }
}
