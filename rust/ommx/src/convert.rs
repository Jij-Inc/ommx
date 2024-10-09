//! Additional trait implementations for generated codes

macro_rules! impl_add_inverse {
    ($lhs:ty, $rhs:ty) => {
        impl ::std::ops::Add<$rhs> for $lhs {
            type Output = $rhs;
            fn add(self, rhs: $rhs) -> Self::Output {
                rhs + self
            }
        }
    };
}

macro_rules! impl_add_from {
    ($lhs:ty, $rhs:ty) => {
        impl ::std::ops::Add<$rhs> for $lhs {
            type Output = $lhs;
            fn add(self, rhs: $rhs) -> Self::Output {
                self + <$lhs>::from(rhs)
            }
        }
    };
}

macro_rules! impl_sub_by_neg_add {
    ($lhs:ty, $rhs:ty) => {
        impl ::std::ops::Sub<$rhs> for $lhs {
            type Output = $lhs;
            fn sub(self, rhs: $rhs) -> Self::Output {
                self + (-rhs)
            }
        }
    };
}

macro_rules! impl_mul_inverse {
    ($lhs:ty, $rhs:ty) => {
        impl ::std::ops::Mul<$rhs> for $lhs {
            type Output = $rhs;
            fn mul(self, rhs: $rhs) -> Self::Output {
                rhs * self
            }
        }
    };
}

macro_rules! impl_mul_from {
    ($lhs:ty, $rhs:ty, $output:ty) => {
        impl ::std::ops::Mul<$rhs> for $lhs {
            type Output = $output;
            fn mul(self, rhs: $rhs) -> Self::Output {
                self * <$lhs>::from(rhs)
            }
        }
    };
}

macro_rules! impl_neg_by_mul {
    ($ty:ty) => {
        impl ::std::ops::Neg for $ty {
            type Output = $ty;
            fn neg(self) -> Self::Output {
                self * -1.0
            }
        }
    };
}

mod function;
mod linear;
mod polynomial;
mod quadratic;

use crate::v1::State;
use std::collections::HashMap;

impl From<HashMap<u64, f64>> for State {
    fn from(entries: HashMap<u64, f64>) -> Self {
        Self { entries }
    }
}
