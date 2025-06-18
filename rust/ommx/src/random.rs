//! Random generation and [`mod@proptest`] support for OMMX Message structs
//!
//! Random Generation
//! -----------------
//! The messages like [`crate::v1::Instance`] and [`crate::v1::Linear`] can be generated randomly via [`Arbitrary`] trait
//! using [`random`] and [`random_deterministic`] functions.
//!
//! ```rust
//! use ommx::{v1::{Linear, Instance, decision_variable::Kind}, random::*};
//!
//! // Linear function with random coefficients
//! let linear: Linear = random_deterministic(FunctionParameters { num_terms: 5, max_degree: 1, max_id: 10 });
//!
//! // LP instance
//! let instance: Instance = random_deterministic(InstanceParameters {
//!   num_constraints: 7,
//!   objective: FunctionParameters { max_degree: 1, ..Default::default() },
//!   constraint: FunctionParameters { max_degree: 1, ..Default::default() },
//!   kinds: vec![Kind::Continuous],
//! });
//! ```
//!
//! [`InstanceParameters`] and [`FunctionParameters`] are used to specify the size of the generated components.
//!
//! Proptest Support
//! ----------------
//!
//! This modules implements [`Arbitrary`] trait for the most of structs in [`crate::v1`] module.
//! In addition, there are several helper functions, e.g. [`arbitrary_coefficient`] or [`arbitrary_decision_variables`],
//! for property-based testing by the users of this crate.
//! See [proptest book](https://proptest-rs.github.io/proptest/intro.html) for the details.
//!

mod constraint;
mod decision_variable;
mod function;
mod instance;
mod linear;
mod parameter;
mod parametric_instance;
mod polynomial;
mod quadratic;
mod samples;
mod state;

pub use constraint::*;
pub use decision_variable::*;
pub use function::*;
pub use instance::*;
pub use parameter::*;
pub use samples::*;
pub use state::*;

pub use proptest::test_runner::TestRunner as Rng;

use crate::{v1::State, Bound, Bounds, MonomialDyn, VariableID, VariableIDSet};
use proptest::{
    prelude::*,
    strategy::{Strategy, ValueTree},
};
use std::{collections::HashMap, ops::Deref};

/// Get random object based on [`Arbitrary`] trait with its [`Arbitrary::Parameters`].
pub fn random<T: Arbitrary>(rng: &mut Rng, parameters: T::Parameters) -> T {
    sample(rng, T::arbitrary_with(parameters))
}

/// Get random object based on [`Arbitrary`] trait with its [`Arbitrary::Parameters`] in a deterministic way.
pub fn random_deterministic<T: Arbitrary>(parameters: T::Parameters) -> T {
    sample_deterministic(T::arbitrary_with(parameters))
}

pub fn sample<T>(rng: &mut Rng, strategy: impl Strategy<Value = T>) -> T {
    let tree = strategy.new_tree(rng).expect("Failed to create a new tree");
    tree.current()
}

pub fn sample_deterministic<T>(strategy: impl Strategy<Value = T>) -> T {
    let mut runner = Rng::deterministic();
    let tree = strategy
        .new_tree(&mut runner)
        .expect("Failed to create a new tree");
    tree.current()
}

/// Strategy for generating arbitrary coefficients.
pub fn arbitrary_coefficient() -> BoxedStrategy<f64> {
    prop_oneof![Just(0.0), Just(1.0), Just(-1.0), -1.0..1.0].boxed()
}

pub fn arbitrary_coefficient_nonzero() -> BoxedStrategy<f64> {
    prop_oneof![Just(1.0), Just(-1.0), -1.0..1.0]
        .prop_filter("nonzero", |x: &f64| x.abs() > f64::EPSILON)
        .boxed()
}

/// Generate a strategy for producing a vector of unique integers within a given range `min_id..=max_id`
pub fn unique_integers(min_id: u64, max_id: u64, size: usize) -> BoxedStrategy<Vec<u64>> {
    assert!(
        min_id <= max_id,
        "min_id({min_id}) must be less than or equal to max_id({max_id}) to ensure a valid range"
    );
    if size as u64 == max_id - min_id + 1 {
        // Only one possible vector
        return Just((min_id..=max_id).collect::<Vec<u64>>()).boxed();
    }
    assert!(
        size <= (max_id - min_id) as usize + 1,
        "size({size}) must be less than or equal to max_id({max_id}) - min_id({min_id}) + 1 to ensure unique ids"
    );
    if size == 0 {
        return Just(Vec::new()).boxed();
    }
    (min_id..=(max_id - size as u64 + 1))
        .prop_flat_map(move |head| {
            if size == 1 {
                return Just(vec![head]).boxed();
            }
            unique_integers(head + 1, max_id, size - 1)
                .prop_map(move |mut tail| {
                    tail.insert(0, head);
                    tail
                })
                .boxed()
        })
        .boxed()
}

/// Generate unique pairs of integers `(i, j)` where `i <= j <= max_id`
pub fn unique_integer_pairs(max_id: u64, num_terms: usize) -> BoxedStrategy<Vec<(u64, u64)>> {
    unique_integers(0, multi_choose(max_id + 1, 2) - 1, num_terms)
        .prop_map(move |ids| {
            ids.into_iter()
                .map(|k| {
                    let tuple = map_k_to_tuple(k, 2, max_id);
                    (tuple[0], tuple[1])
                })
                .collect()
        })
        .boxed()
}

pub fn unique_sorted_ids(
    max_id: u64,
    degree: usize,
    num_terms: usize,
) -> BoxedStrategy<Vec<MonomialDyn>> {
    unique_integers(0, multi_choose(max_id + 1, degree) - 1, num_terms)
        .prop_map(move |ids| {
            ids.into_iter()
                .map(|k| {
                    let tuple = map_k_to_tuple(k, degree, max_id)
                        .into_iter()
                        .map(VariableID::from)
                        .collect();
                    MonomialDyn::new(tuple)
                })
                .collect()
        })
        .boxed()
}

/// Introduce lex order for `(i1, i2, ..., iD)` to a unique integer `k` to use `unique_integers` strategy
fn map_k_to_tuple(k: u64, dim: usize, n: u64) -> Vec<u64> {
    let mut result = Vec::with_capacity(dim);
    let mut remaining_k = k;
    for i in 0..dim {
        let rdim = dim - i - 1;
        let mut current_digit = result.last().copied().unwrap_or(0);
        loop {
            let h = multi_choose(n - current_digit + 1, rdim);
            if remaining_k < h {
                break;
            }
            remaining_k -= h;
            current_digit += 1;
        }
        result.push(current_digit);
    }
    result
}

/// nCr
fn combinations(n: u64, r: usize) -> u64 {
    if r as u64 > n {
        return 0;
    }
    if r == 0 || r as u64 == n {
        return 1;
    }
    if r > (n / 2) as usize {
        return combinations(n, n as usize - r);
    }
    let mut res = 1;
    for i in 0..r {
        res = res * (n - i as u64) / (i as u64 + 1);
    }
    res
}

/// nHr
pub fn multi_choose(n: u64, r: usize) -> u64 {
    combinations(n + r as u64 - 1, r)
}

pub fn arbitrary_split_ids(ids: VariableIDSet) -> BoxedStrategy<(VariableIDSet, VariableIDSet)> {
    let flips = proptest::collection::vec(bool::arbitrary(), ids.len());
    flips
        .prop_map(move |flips| {
            let mut used_ids = VariableIDSet::default();
            let mut defined_ids = VariableIDSet::default();
            for (flip, id) in flips.into_iter().zip(ids.iter()) {
                if flip {
                    used_ids.insert(*id);
                } else {
                    defined_ids.insert(*id);
                }
            }
            (used_ids, defined_ids)
        })
        .boxed()
}

pub fn arbitrary_split_state(state: &State) -> BoxedStrategy<(State, State)> {
    let ids: Vec<(u64, f64)> = state.entries.iter().map(|(k, v)| (*k, *v)).collect();
    let flips = proptest::collection::vec(bool::arbitrary(), ids.len());
    (Just(ids), flips)
        .prop_map(|(ids, flips)| {
            let mut a = State::default();
            let mut b = State::default();
            for (flip, (id, value)) in flips.into_iter().zip(ids.into_iter()) {
                if flip {
                    a.entries.insert(id, value);
                } else {
                    b.entries.insert(id, value);
                }
            }
            (a, b)
        })
        .boxed()
}

pub fn arbitrary_bounds(ids: impl Iterator<Item = VariableID>) -> BoxedStrategy<Bounds> {
    let mut strategy = Just(Bounds::default()).boxed();
    for id in ids {
        strategy = (strategy, Bound::arbitrary())
            .prop_map(move |(mut bounds, bound)| {
                bounds.insert(id, bound);
                bounds
            })
            .boxed();
    }
    strategy
}

pub fn arbitrary_state_within_bounds(bounds: &Bounds, max_abs: f64) -> BoxedStrategy<State> {
    let mut stratety = Just(HashMap::new()).boxed();
    for (id, bound) in bounds {
        let raw_id = *id.deref();
        stratety = (stratety, bound.arbitrary_containing(max_abs))
            .prop_map(move |(mut state, value)| {
                state.insert(raw_id, value);
                state
            })
            .boxed();
    }
    stratety.prop_map(|state| state.into()).boxed()
}

/// Generate random partitions of a sum into n parts at least 1 each.
///
/// For example, if `sum = 5` and `n = 3`, it can generate:
/// `[1, 1, 3]`, `[1, 2, 2]`, `[2, 1, 2]`, `[3, 1, 1]`, `[2, 2, 1]`, etc.
pub fn arbitrary_integer_partition(sum: usize, n: usize) -> BoxedStrategy<Vec<usize>> {
    if n == 1 {
        return Just(vec![sum]).boxed();
    }
    if sum == n {
        return Just(vec![1; n]).boxed();
    }
    if sum < n {
        panic!("sum({sum}) cannot be split into {n} positive parts");
    }
    (1..(sum - n))
        .prop_flat_map(move |x| {
            arbitrary_integer_partition(sum - x, n - 1).prop_map(move |mut sub| {
                sub.push(x);
                sub
            })
        })
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[should_panic]
    #[test]
    fn test_unique_integers_panic_too_large_size() {
        let _ = unique_integers(0, 1, 3);
    }

    #[should_panic]
    #[test]
    fn test_unique_integers_panic_invalid_range() {
        let _ = unique_integers(1, 0, 1);
    }

    proptest! {
        #[test]
        fn test_unique_integers(ids in unique_integers(0, 5, 3)) {
            prop_assert_eq!(ids.len(), 3);
            prop_assert_eq!(ids.iter().cloned().collect::<std::collections::HashSet<_>>().len(), 3);
            prop_assert!(ids.iter().all(|&id| id <= 5));
        }
    }

    #[test]
    fn test_unique_integers_recursion_limit() {
        let size = 100000_usize;
        let strategy = unique_integers(0, 10 * size as u64, size);
        let mut runner = proptest::test_runner::TestRunner::deterministic();
        let tree = strategy
            .new_tree(&mut runner)
            .expect("Failed to create a new tree");
        let ids = tree.current();
        println!("{:?}", ids);
    }

    #[test]
    fn test_multichoose() {
        let n = 5;
        assert_eq!(multi_choose(n, 2), 5 * 6 / 2);
    }

    #[test]
    fn test_map_k_to_tuple_2d() {
        assert_eq!(multi_choose(3 + 1, 2), 10);
        assert_eq!(map_k_to_tuple(0, 2, 3), vec![0, 0]);
        assert_eq!(map_k_to_tuple(1, 2, 3), vec![0, 1]);
        assert_eq!(map_k_to_tuple(2, 2, 3), vec![0, 2]);
        assert_eq!(map_k_to_tuple(3, 2, 3), vec![0, 3]);
        assert_eq!(map_k_to_tuple(4, 2, 3), vec![1, 1]);
        assert_eq!(map_k_to_tuple(5, 2, 3), vec![1, 2]);
        assert_eq!(map_k_to_tuple(6, 2, 3), vec![1, 3]);
        assert_eq!(map_k_to_tuple(7, 2, 3), vec![2, 2]);
        assert_eq!(map_k_to_tuple(8, 2, 3), vec![2, 3]);
        assert_eq!(map_k_to_tuple(9, 2, 3), vec![3, 3]);
    }

    #[test]
    fn test_map_k_to_tuple_3d() {
        assert_eq!(multi_choose(3 + 1, 3), 20);
        assert_eq!(map_k_to_tuple(0, 3, 3), vec![0, 0, 0]);
        assert_eq!(map_k_to_tuple(1, 3, 3), vec![0, 0, 1]);
        assert_eq!(map_k_to_tuple(2, 3, 3), vec![0, 0, 2]);
        assert_eq!(map_k_to_tuple(3, 3, 3), vec![0, 0, 3]);
        assert_eq!(map_k_to_tuple(4, 3, 3), vec![0, 1, 1]);
        assert_eq!(map_k_to_tuple(5, 3, 3), vec![0, 1, 2]);
        assert_eq!(map_k_to_tuple(6, 3, 3), vec![0, 1, 3]);
        assert_eq!(map_k_to_tuple(7, 3, 3), vec![0, 2, 2]);
        assert_eq!(map_k_to_tuple(8, 3, 3), vec![0, 2, 3]);
        assert_eq!(map_k_to_tuple(9, 3, 3), vec![0, 3, 3]);
        assert_eq!(map_k_to_tuple(10, 3, 3), vec![1, 1, 1]);
        assert_eq!(map_k_to_tuple(11, 3, 3), vec![1, 1, 2]);
        assert_eq!(map_k_to_tuple(12, 3, 3), vec![1, 1, 3]);
        assert_eq!(map_k_to_tuple(13, 3, 3), vec![1, 2, 2]);
        assert_eq!(map_k_to_tuple(14, 3, 3), vec![1, 2, 3]);
        assert_eq!(map_k_to_tuple(15, 3, 3), vec![1, 3, 3]);
        assert_eq!(map_k_to_tuple(16, 3, 3), vec![2, 2, 2]);
        assert_eq!(map_k_to_tuple(17, 3, 3), vec![2, 2, 3]);
        assert_eq!(map_k_to_tuple(18, 3, 3), vec![2, 3, 3]);
        assert_eq!(map_k_to_tuple(19, 3, 3), vec![3, 3, 3]);
    }

    proptest! {
        #[test]
        fn test_arbitrary_integer_partition(partition in arbitrary_integer_partition(5, 3)) {
            prop_assert_eq!(partition.len(), 3);
            prop_assert_eq!(partition.iter().sum::<usize>(), 5);
            prop_assert!(partition.iter().all(|&x| x > 0));
        }
    }
}
