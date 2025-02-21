use crate::v1::{DecisionVariable, Function, Linear, Polynomial, Quadratic};
use std::ops::*;

impl From<&DecisionVariable> for Linear {
    fn from(dv: &DecisionVariable) -> Self {
        Linear::from(dv.id)
    }
}

macro_rules! impl_from_decision_variable {
    ($type:ty) => {
        impl From<&DecisionVariable> for $type {
            fn from(dv: &DecisionVariable) -> Self {
                Linear::from(dv).into()
            }
        }
    };
}
impl_from_decision_variable!(Quadratic);
impl_from_decision_variable!(Polynomial);
impl_from_decision_variable!(Function);

impl Add for &DecisionVariable {
    type Output = Linear;
    fn add(self, rhs: Self) -> Self::Output {
        Linear::from(self) + Linear::from(rhs)
    }
}

macro_rules! impl_add_decision_variable {
    ($t:ty) => {
        impl Add<$t> for &DecisionVariable {
            type Output = <Linear as Add<$t>>::Output;
            fn add(self, rhs: $t) -> Self::Output {
                Linear::from(self) + rhs
            }
        }
        impl Add<&DecisionVariable> for $t {
            type Output = <Linear as Add<$t>>::Output;
            fn add(self, rhs: &DecisionVariable) -> Self::Output {
                self + Linear::from(rhs)
            }
        }
    };
}
impl_add_decision_variable!(f64);
impl_add_decision_variable!(Linear);
impl_add_decision_variable!(Quadratic);
impl_add_decision_variable!(Polynomial);
impl_add_decision_variable!(Function);

impl Mul for &DecisionVariable {
    type Output = Quadratic;

    fn mul(self, rhs: Self) -> Self::Output {
        Linear::from(self) * Linear::from(rhs)
    }
}

macro_rules! impl_mul_decision_variable {
    ($t:ty) => {
        impl Mul<$t> for &DecisionVariable {
            type Output = <Linear as Mul<$t>>::Output;
            fn mul(self, rhs: $t) -> Self::Output {
                Linear::from(self) * rhs
            }
        }
        impl Mul<&DecisionVariable> for $t {
            type Output = <Linear as Mul<$t>>::Output;
            fn mul(self, rhs: &DecisionVariable) -> Self::Output {
                self * Linear::from(rhs)
            }
        }
    };
}
impl_mul_decision_variable!(f64);
impl_mul_decision_variable!(Linear);
impl_mul_decision_variable!(Quadratic);
impl_mul_decision_variable!(Polynomial);
impl_mul_decision_variable!(Function);

impl Neg for &DecisionVariable {
    type Output = Linear;

    fn neg(self) -> Self::Output {
        -Linear::from(self)
    }
}
