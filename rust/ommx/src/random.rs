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
