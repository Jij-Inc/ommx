//! Random generation and [`proptest`] support for OMMX Message structs
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
//! let linear = random_deterministic(LinearParameters { num_terms: 5, max_id: 10 });
//!
//! // LP instance
//! let instance = random_deterministic(InstanceParameters {
//!   num_constraints: 7,
//!   num_terms: 5,
//!   max_degree: 1,
//!   max_id: 10
//! });
//! ```
//!
//! [`InstanceParameters`] and [`LinearParameters`] are used to specify the size of the generated components.
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
