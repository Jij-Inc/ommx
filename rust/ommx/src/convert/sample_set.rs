use crate::v1::{SampledValues, Samples, State};
use std::collections::HashMap;

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

    /// Transpose `sample_id -> decision_variable_id -> value` to `decision_variable_id -> sample_id -> value`
    pub fn transpose(&self) -> HashMap<u64, SampledValues> {
        let mut out = HashMap::new();
        for (sample_id, state) in self.iter() {
            for (decision_variable_id, value) in state.entries.iter() {
                out.entry(*decision_variable_id)
                    .or_insert_with(SampledValues::default)
                    .values
                    .insert(*sample_id, *value);
            }
        }
        out
    }
}
