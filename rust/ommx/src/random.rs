//! Randomly generate OMMX components for benchmarking and testing

use proptest::{
    prelude::*,
    strategy::{Strategy, ValueTree},
    test_runner::TestRunner,
};

mod decision_variable;
mod function;
mod linear;
mod polynomial;
mod quadratic;

pub use decision_variable::*;
pub use function::*;
pub use linear::*;
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
