//! Randomly generate OMMX components for benchmarking and testing

use crate::v1::{
    self, decision_variable::Kind, linear::Term, Bound, Constraint, DecisionVariable, Equality,
};
use rand::Rng;

/// Create a random linear programming (LP) instance in a form of `min c^T x` subject to `Ax = b` and `x >= 0` with continuous variables `x`.
pub fn random_lp(rng: &mut impl Rng, num_variables: usize, num_constraints: usize) -> v1::Instance {
    let decision_variables = (0..num_variables)
        .map(|i| DecisionVariable {
            id: i as u64,
            kind: Kind::Continuous as i32,
            name: Some("x".into()),
            subscripts: vec![i as i64],
            bound: Some(Bound {
                lower: 0.0,
                upper: f64::INFINITY,
            }),
            ..Default::default()
        })
        .collect();
    let mut instance = v1::Instance {
        decision_variables,
        ..Default::default()
    };
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
