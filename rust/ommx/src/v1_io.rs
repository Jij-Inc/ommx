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
                    .is_some_and(|v| atol.approx_eq(*value, *v))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_abs_diff_eq_uses_the_inclusive_atol_primitive() {
        let atol = ATol::new(0.125).unwrap();
        let reference = State::from_iter([(1, 1.0), (2, -1.0)]);
        let boundary = State::from_iter([(1, 1.125), (2, -1.125)]);
        let outside = State::from_iter([(1, f64::from_bits(1.125_f64.to_bits() + 1)), (2, -1.0)]);

        assert!(reference.abs_diff_eq(&boundary, atol));
        assert!(!reference.abs_diff_eq(&outside, atol));

        let non_finite = State::from_iter([(1, f64::INFINITY), (2, -1.0)]);
        assert!(!non_finite.abs_diff_eq(&non_finite, ATol::new(f64::INFINITY).unwrap()));
    }
}
