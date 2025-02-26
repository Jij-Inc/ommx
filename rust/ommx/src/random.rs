//! Random generation and [`mod@proptest`] support for OMMX Message structs
//!
//! Random Generation
//! -----------------
//! The messages like [`crate::v1::Instance`] and [`crate::v1::Linear`] can be generated randomly via [`Arbitrary`] trait
//! using [`random`] and [`random_deterministic`] functions.
//!
//! ```rust
//! use ommx::{v1, random::*};
//!
//! // Linear function with random coefficients
//! let linear: v1::Linear = random_deterministic(LinearParameters { num_terms: 5, max_id: 10 });
//!
//! // LP instance
//! let instance: v1::Instance = random_deterministic(InstanceParameters {
//!   num_constraints: 7,
//!   num_terms: 5,
//!   max_degree: 1,
//!   max_id: 10
//! });
//! ```
//!
//! [`InstanceParameters`] and [`LinearParameters`] are used to specify the size of the generated components.
//!
//! Proptest Support
//! ----------------
//!
//! This modules implements [`Arbitrary`] trait for the most of structs in [`crate::v1`] module.
//! In addition, there are several helper functions, e.g. [`arbitrary_coefficient`] or [`arbitrary_decision_variables`],
//! for property-based testing by the users of this crate.
//! See [proptest book](https://proptest-rs.github.io/proptest/intro.html) for the details.
//!

use proptest::{
    prelude::*,
    strategy::{Strategy, ValueTree},
    test_runner::TestRunner,
};

mod constraint;
mod decision_variable;
mod function;
mod instance;
mod linear;
mod parameter;
mod parametric_instance;
mod polynomial;
mod quadratic;
mod state;

pub use constraint::*;
pub use decision_variable::*;
pub use function::*;
pub use instance::*;
pub use linear::*;
pub use parameter::*;
pub use parametric_instance::*;
pub use polynomial::*;
pub use quadratic::*;

/// Get random object based on [`Arbitrary`] trait with its [`Arbitrary::Parameters`].
pub fn random<T: Arbitrary>(rng: proptest::test_runner::TestRng, parameters: T::Parameters) -> T {
    let strategy = T::arbitrary_with(parameters);
    let config = proptest::test_runner::Config::default();
    let mut runner = proptest::test_runner::TestRunner::new_with_rng(config, rng);
    let tree = strategy
        .new_tree(&mut runner)
        .expect("Failed to create a new tree");
    tree.current()
}

/// Get random object based on [`Arbitrary`] trait with its [`Arbitrary::Parameters`] in a deterministic way.
pub fn random_deterministic<T: Arbitrary>(parameters: T::Parameters) -> T {
    let strategy = T::arbitrary_with(parameters);
    let mut runner = TestRunner::deterministic();
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

// Only samples where `num_terms <= max_id + 1`
fn num_terms_and_max_id(num_terms: usize, max_id: u64) -> impl Strategy<Value = (usize, u64)> {
    (0..=max_id).prop_flat_map(move |max_id| {
        let max_num_terms = std::cmp::min(max_id as usize + 1, num_terms);
        (0..=max_num_terms).prop_map(move |num_terms| (num_terms, max_id))
    })
}

/// Generate a strategy for producing a vector of unique integers within a given range `min_id..=max_id`
fn unique_integers(min_id: u64, max_id: u64, size: usize) -> BoxedStrategy<Vec<u64>> {
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
fn unique_integer_pairs(max_id: u64, num_terms: usize) -> BoxedStrategy<Vec<(u64, u64)>> {
    // Map `(i, j)` to a unique integer `k = i * (2 * n - i + 3) / 2 + j`
    unique_integers(0, (max_id + 1) * (max_id + 2) / 2 - 1, num_terms)
        .prop_map(move |ids| ids.into_iter().map(|k| map_k_to_ij(k, max_id)).collect())
        .boxed()
}

fn map_k_to_ij(k: u64, n: u64) -> (u64, u64) {
    let i = ((-2.0 * n as f64 - 3.0 + (((2.0 * n as f64 + 3.0).powi(2) - 8.0 * k as f64).sqrt()))
        / -2.0)
        .floor() as u64;
    let start_k = i * (2 * n - i + 3) / 2;
    let j = k - start_k + i;
    (i, j)
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
}
