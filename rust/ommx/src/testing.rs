//! Randomly generate OMMX components for benchmarking and testing

use crate::v1::{self, constraint::Equality, linear::Term, Constraint};
use rand::Rng;

/// Create a random linear programming instance in a form of `min c^T x` subject to `Ax = b` and `x >= 0` with continuous variables `x`.
pub fn random_lp_instance(rng: &mut impl Rng) -> v1::Instance {
    let num_variables = rng.gen_range(1..=10);
    let num_constraints = rng.gen_range(1..=10);
    let mut instance = v1::Instance::default();
    for constraint_id in 0..num_constraints {
        let mut linear = v1::Linear::default();
        for id in 0..num_variables {
            // A
            linear.terms.push(Term {
                id,
                coefficient: rng.gen_range(-1.0..1.0),
            });
            // -b
            linear.constant = rng.gen_range(-1.0..1.0);
        }
        instance.constraints.push(Constraint {
            id: constraint_id,
            equality: Equality::EqualToZero as i32,
            function: Some(linear.into()),
            description: None,
        });
    }
    let mut objective = v1::Linear::default();
    for id in 0..num_variables {
        // c
        objective.terms.push(Term {
            id,
            coefficient: rng.gen_range(-1.0..1.0),
        });
    }
    instance.objective = Some(objective.into());

    instance
}
