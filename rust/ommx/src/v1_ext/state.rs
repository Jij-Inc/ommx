use crate::v1::State;
use std::collections::HashMap;

impl From<HashMap<u64, f64>> for State {
    fn from(value: HashMap<u64, f64>) -> Self {
        Self { entries: value }
    }
}

impl FromIterator<(u64, f64)> for State {
    fn from_iter<T: IntoIterator<Item = (u64, f64)>>(iter: T) -> Self {
        Self {
            entries: iter.into_iter().collect(),
        }
    }
}

impl IntoIterator for State {
    type Item = (u64, f64);
    type IntoIter = std::collections::hash_map::IntoIter<u64, f64>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}
