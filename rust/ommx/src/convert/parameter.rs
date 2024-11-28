use crate::v1::{Function, Linear, Parameter, Polynomial, Quadratic};
use std::ops::*;

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
