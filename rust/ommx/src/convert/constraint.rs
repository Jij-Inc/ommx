use crate::v1::{Constraint, Equality, Function};
use anyhow::{Context, Result};
use proptest::prelude::*;

impl Constraint {
    pub fn function(&self) -> Result<&Function> {
        self.function
            .as_ref()
            .context("Constraint does not contain function")
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
