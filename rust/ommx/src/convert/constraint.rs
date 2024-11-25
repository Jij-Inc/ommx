use crate::v1::{Constraint, Equality, Function};
use anyhow::{Context, Result};
use approx::AbsDiffEq;
use proptest::prelude::*;

impl Constraint {
    pub fn function(&self) -> Result<&Function> {
        self.function
            .as_ref()
            .context("Constraint does not contain function")
    }
}

impl AbsDiffEq for Constraint {
    type Epsilon = f64;

    fn default_epsilon() -> Self::Epsilon {
        f64::EPSILON
    }

    fn abs_diff_eq(&self, other: &Self, epsilon: Self::Epsilon) -> bool {
        if self.equality != other.equality {
            return false;
        }
        if let (Some(f), Some(g)) = (&self.function, &other.function) {
            f.abs_diff_eq(g, epsilon)
        } else {
            false
        }
    }
}

impl Arbitrary for Constraint {
    type Parameters = <Function as Arbitrary>::Parameters;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(parameters: Self::Parameters) -> Self::Strategy {
        let function = Function::arbitrary_with(parameters);
        let equality = prop_oneof![
            Just(Equality::EqualToZero),
            Just(Equality::LessThanOrEqualToZero)
        ];
        (function, equality)
            .prop_map(|(function, equality)| Constraint {
                id: 0, // ID should be changed when creating an instance
                function: Some(function),
                equality: equality as i32,
                ..Default::default()
            })
            .boxed()
    }

    fn arbitrary() -> Self::Strategy {
        (0..10_usize, 0..5_usize, 0..10_u64)
            .prop_flat_map(Self::arbitrary_with)
            .boxed()
    }
}

pub fn arbitrary_constraints(
    num_constraints: usize,
    parameters: <Constraint as Arbitrary>::Parameters,
) -> BoxedStrategy<Vec<Constraint>> {
    let constraints =
        proptest::collection::vec(Constraint::arbitrary_with(parameters), num_constraints);
    let constraint_ids = prop_oneof![
        // continuous case
        Just((0..(num_constraints as u64)).collect::<Vec<u64>>()).prop_shuffle(),
        // discrete case
        Just((0..(3 * num_constraints as u64)).collect::<Vec<u64>>()).prop_shuffle(),
    ];
    (constraints, constraint_ids)
        .prop_map(|(mut c, id)| {
            for (id, c) in id.iter().zip(c.iter_mut()) {
                c.id = *id;
            }
            c
        })
        .boxed()
}
