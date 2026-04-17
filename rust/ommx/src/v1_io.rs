//! IO-adjacent conversions on protobuf-generated `v1::*` types.
//!
//! These impls are the last surviving inhabitants of what used to be
//! `rust/ommx/src/v1_ext/`. They are kept because several test fixtures and
//! a handful of production call sites construct `v1::State` from a
//! `HashMap<u64, f64>` literal — a bag-of-bytes shape that is hard to
//! express through the generated protobuf API alone. Everything else that
//! used to live in `v1_ext/` was either ported to the domain layer or
//! deleted with its only (internal) callers.

use crate::{v1::State, ATol};
use approx::AbsDiffEq;
use std::collections::HashMap;

impl From<HashMap<u64, f64>> for State {
    fn from(entries: HashMap<u64, f64>) -> Self {
        Self { entries }
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

impl AbsDiffEq for State {
    type Epsilon = ATol;

    fn default_epsilon() -> Self::Epsilon {
        ATol::default()
    }

    fn abs_diff_eq(&self, other: &Self, atol: Self::Epsilon) -> bool {
        self.entries.len() == other.entries.len()
            && self.entries.iter().all(|(key, value)| {
                other
                    .entries
                    .get(key)
                    .is_some_and(|v| (*value - *v).abs() < atol)
            })
    }
}
