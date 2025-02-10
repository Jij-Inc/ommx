use crate::v1::{DecisionVariable, Function, Linear, Parameter, Polynomial, Quadratic};
use proptest::prelude::*;
use std::{
    collections::{BTreeSet, HashMap},
    ops::*,
};

impl From<&Parameter> for Linear {
    fn from(dv: &Parameter) -> Self {
        Linear::from(dv.id)
    }
}

macro_rules! impl_from_parameter {
    ($type:ty) => {
        impl From<&Parameter> for $type {
            fn from(dv: &Parameter) -> Self {
                Linear::from(dv).into()
            }
        }
    };
}
impl_from_parameter!(Quadratic);
impl_from_parameter!(Polynomial);
impl_from_parameter!(Function);

impl Add for &Parameter {
    type Output = Linear;
    fn add(self, rhs: Self) -> Self::Output {
        Linear::from(self) + Linear::from(rhs)
    }
}

impl Add<&DecisionVariable> for &Parameter {
    type Output = Linear;
    fn add(self, rhs: &DecisionVariable) -> Self::Output {
        Linear::from(self) + Linear::from(rhs)
    }
}

impl Add<&Parameter> for &DecisionVariable {
    type Output = Linear;
    fn add(self, rhs: &Parameter) -> Self::Output {
        Linear::from(self) + Linear::from(rhs)
    }
}

macro_rules! impl_add_parameter {
    ($t:ty) => {
        impl Add<$t> for &Parameter {
            type Output = <Linear as Add<$t>>::Output;
            fn add(self, rhs: $t) -> Self::Output {
                Linear::from(self) + rhs
            }
        }
        impl Add<&Parameter> for $t {
            type Output = <Linear as Add<$t>>::Output;
            fn add(self, rhs: &Parameter) -> Self::Output {
                self + Linear::from(rhs)
            }
        }
    };
}
impl_add_parameter!(f64);
impl_add_parameter!(Linear);
impl_add_parameter!(Quadratic);
impl_add_parameter!(Polynomial);
impl_add_parameter!(Function);

impl Mul for &Parameter {
    type Output = Quadratic;

    fn mul(self, rhs: Self) -> Self::Output {
        Linear::from(self) * Linear::from(rhs)
    }
}

impl Mul<&DecisionVariable> for &Parameter {
    type Output = Quadratic;
    fn mul(self, rhs: &DecisionVariable) -> Self::Output {
        Linear::from(self) * Linear::from(rhs)
    }
}

impl Mul<&Parameter> for &DecisionVariable {
    type Output = Quadratic;
    fn mul(self, rhs: &Parameter) -> Self::Output {
        Linear::from(self) * Linear::from(rhs)
    }
}

macro_rules! impl_mul_parameter {
    ($t:ty) => {
        impl Mul<$t> for &Parameter {
            type Output = <Linear as Mul<$t>>::Output;
            fn mul(self, rhs: $t) -> Self::Output {
                Linear::from(self) * rhs
            }
        }
        impl Mul<&Parameter> for $t {
            type Output = <Linear as Mul<$t>>::Output;
            fn mul(self, rhs: &Parameter) -> Self::Output {
                self * Linear::from(rhs)
            }
        }
    };
}
impl_mul_parameter!(f64);
impl_mul_parameter!(Linear);
impl_mul_parameter!(Quadratic);
impl_mul_parameter!(Polynomial);
impl_mul_parameter!(Function);

impl Neg for &Parameter {
    type Output = Linear;

    fn neg(self) -> Self::Output {
        -Linear::from(self)
    }
}

impl Arbitrary for Parameter {
    type Parameters = u64;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(max_id: Self::Parameters) -> Self::Strategy {
        let subscripts = prop_oneof![
            Just(Vec::<i64>::new()),
            proptest::collection::vec(-(max_id as i64)..=(max_id as i64), 1..=3),
        ];
        let parameters = prop_oneof![
            Just(HashMap::<String, String>::new()),
            proptest::collection::hash_map(".{0,3}", ".{0,3}", 1..=3),
        ];
        (
            0..=max_id,
            proptest::option::of(".{0,3}"),
            subscripts,
            parameters,
            proptest::option::of(".{0,3}"),
        )
            .prop_map(
                |(id, name, subscripts, parameters, description)| Parameter {
                    id,
                    name,
                    subscripts,
                    parameters,
                    description,
                },
            )
            .boxed()
    }
}

pub fn arbitrary_parameters(ids: BTreeSet<u64>) -> BoxedStrategy<Vec<Parameter>> {
    (
        proptest::collection::vec(Parameter::arbitrary(), ids.len()),
        Just(ids),
    )
        .prop_map(|(mut dvs, used_ids)| {
            for (dv, id) in dvs.iter_mut().zip(used_ids.iter()) {
                dv.id = *id;
            }
            dvs
        })
        .boxed()
}
