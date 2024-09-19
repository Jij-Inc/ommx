//! Randomly generate OMMX components for benchmarking and testing

use crate::v1::{
    self, decision_variable::Kind, linear::Term, Bound, Constraint, DecisionVariable, Equality,
};
use rand::Rng;

/// Create a random linear programming (LP) instance in a form of `min c^T x` subject to `Ax = b` and `x >= 0` with continuous variables `x`.
pub fn random_lp(rng: &mut impl Rng, num_variables: usize, num_constraints: usize) -> v1::Instance {
    let mut instance = v1::Instance::default();
    instance.decision_variables = (0..num_variables)
        .map(|i| {
            let mut var = DecisionVariable::default();
            var.id = i as u64;
            var.kind = Kind::Continuous as i32;
            var.name = Some("x".into());
            var.subscripts = vec![i as i64];
            var.bound = Some(Bound {
                lower: 0.0,
                upper: f64::INFINITY,
            });
            var
        })
        .collect();
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
            ..Default::default()
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
