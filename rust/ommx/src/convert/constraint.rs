use crate::v1::{
    Constraint, Equality, EvaluatedConstraint, Function, RemovedConstraint, SampledConstraint,
};
use anyhow::{bail, ensure, Context, Result};
use approx::AbsDiffEq;
use num::Zero;
use proptest::prelude::*;
use std::{borrow::Cow, collections::HashMap};

impl Constraint {
    pub fn function(&self) -> Cow<Function> {
        match &self.function {
            Some(f) => Cow::Borrowed(f),
            // Empty function is regarded as zero function
            None => Cow::Owned(Function::zero()),
        }
    }
}

impl EvaluatedConstraint {
    pub fn is_feasible(&self, atol: f64) -> Result<bool> {
        ensure!(atol > 0.0, "atol must be positive");
        if self.equality() == Equality::EqualToZero {
            return Ok(self.evaluated_value.abs() < atol);
        } else if self.equality() == Equality::LessThanOrEqualToZero {
            return Ok(self.evaluated_value < atol);
        }
        bail!("Unsupported equality: {:?}", self.equality());
    }
}

impl SampledConstraint {
    pub fn is_feasible(&self, atol: f64) -> Result<HashMap<u64, bool>> {
        ensure!(atol > 0.0, "atol must be positive");
        let values = self
            .evaluated_values
            .as_ref()
            .context("evaluated_values of SampledConstraints is lacked")?;
        if self.equality() == Equality::EqualToZero {
            return Ok(values
                .iter()
                .map(|(id, value)| (*id, value.abs() < atol))
                .collect());
        } else if self.equality() == Equality::LessThanOrEqualToZero {
            return Ok(values
                .iter()
                .map(|(id, value)| (*id, *value < atol))
                .collect());
        }
        bail!("Unsupported equality: {:?}", self.equality());
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
        let function = proptest::option::of(Function::arbitrary_with(parameters));
        let equality = prop_oneof![
            Just(Equality::EqualToZero),
            Just(Equality::LessThanOrEqualToZero)
        ];
        (function, equality)
            .prop_map(|(function, equality)| Constraint {
                id: 0, // ID should be changed when creating an instance
                function,
                equality: equality as i32,
                ..Default::default()
            })
            .boxed()
    }

    fn arbitrary() -> Self::Strategy {
        (0..10_usize, 0..5_u32, 0..10_u64)
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

impl Arbitrary for RemovedConstraint {
    type Parameters = <Constraint as Arbitrary>::Parameters;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(parameters: Self::Parameters) -> Self::Strategy {
        (
            Constraint::arbitrary_with(parameters),
            ".{0,3}",
            proptest::collection::hash_map(".{0,3}", ".{0,3}", 0..=2),
        )
            .prop_map(
                |(constraint, removed_reason, removed_reason_parameters)| RemovedConstraint {
                    constraint: Some(constraint),
                    removed_reason,
                    removed_reason_parameters,
                },
            )
            .boxed()
    }

    fn arbitrary() -> Self::Strategy {
        (0..10_usize, 0..5_u32, 0..10_u64)
            .prop_flat_map(Self::arbitrary_with)
            .boxed()
    }
}
