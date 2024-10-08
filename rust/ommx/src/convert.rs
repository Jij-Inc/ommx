//! Additional trait implementations for generated codes

macro_rules! impl_add_inverse {
    ($lhs:ty, $rhs:ty, $output:ty) => {
        impl ::std::ops::Add<$rhs> for $lhs {
            type Output = $output;
            fn add(self, rhs: $rhs) -> Self::Output {
                rhs + self
            }
        }
    };
}

macro_rules! impl_mul_inverse {
    ($lhs:ty, $rhs:ty, $output:ty) => {
        impl ::std::ops::Mul<$rhs> for $lhs {
            type Output = $output;
            fn mul(self, rhs: $rhs) -> Self::Output {
                rhs * self
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
