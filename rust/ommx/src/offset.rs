use ordered_float::NotNan;
use proptest::prelude::*;
use std::ops::{Add, AddAssign, Deref, Mul, MulAssign};

use crate::Coefficient;

#[derive(Debug, thiserror::Error)]
pub enum OffsetError {
    #[error("Offset must be finite")]
    Infinite,
    #[error("Offset must be not NaN")]
    NaN,
}

/// Offset of polynomial
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(transparent)]
pub struct Offset(NotNan<f64>);

impl Offset {
    pub fn into_inner(self) -> f64 {
        self.0.into_inner()
    }
}

impl From<Offset> for f64 {
    fn from(offset: Offset) -> Self {
        offset.into_inner()
    }
}

impl Deref for Offset {
    type Target = f64;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl TryFrom<f64> for Offset {
    type Error = OffsetError;
    fn try_from(value: f64) -> Result<Self, Self::Error> {
        if value.is_nan() {
            return Err(OffsetError::NaN);
        }
        if !value.is_finite() {
            return Err(OffsetError::Infinite);
        }
        Ok(Self(NotNan::new(value).unwrap())) // Safe because we checked the value is not NaN
    }
}

impl From<Coefficient> for Offset {
    fn from(value: Coefficient) -> Self {
        Self(NotNan::new(value.into_inner()).unwrap()) // Coefficient is stricter than Offset
    }
}

impl Add for Offset {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Offset(self.0 + rhs.0)
    }
}

impl AddAssign for Offset {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl Mul for Offset {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        Offset(self.0 * rhs.0)
    }
}

impl MulAssign for Offset {
    fn mul_assign(&mut self, rhs: Self) {
        self.0 *= rhs.0;
    }
}

impl Arbitrary for Offset {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;
    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        prop_oneof![Just(0.0), Just(1.0), Just(-1.0), -10.0..10.0]
            .prop_map(|x| Offset::try_from(x).unwrap())
            .boxed()
    }
}
