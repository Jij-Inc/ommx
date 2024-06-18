//! Randomly generate OMMX components for benchmarking and testing

use crate::v1::{self, linear::Term, Constraint, Equality};
use rand::Rng;

/// Create a random linear programming (LP) instance in a form of `min c^T x` subject to `Ax = b` and `x >= 0` with continuous variables `x`.
pub fn random_lp(rng: &mut impl Rng, num_variables: usize, num_constraints: usize) -> v1::Instance {
    let mut instance = v1::Instance::default();
    for constraint_id in 0..num_constraints {
        let mut linear = v1::Linear::default();
        for id in 0..num_variables {
            // A
            linear.terms.push(Term {
                id: id as u64,
                coefficient: rng.gen_range(-1.0..1.0),
            });
            // -b
            linear.constant = rng.gen_range(-1.0..1.0);
        }
        instance.constraints.push(Constraint {
            id: constraint_id as u64,
            equality: Equality::EqualToZero as i32,
            function: Some(linear.into()),
            title: None,
            parameters: Default::default(),
            description: None,
        });
    }
    let mut objective = v1::Linear::default();
    for id in 0..num_variables {
        // c
        objective.terms.push(Term {
            id: id as u64,
            coefficient: rng.gen_range(-1.0..1.0),
        });
    }
    instance.objective = Some(objective.into());

    instance
}
