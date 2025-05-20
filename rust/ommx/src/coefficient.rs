use ordered_float::NotNan;
use proptest::prelude::*;
use std::ops::{Add, Deref, Mul, MulAssign, Neg, Sub};

#[derive(Debug, thiserror::Error)]
pub enum CoefficientError {
    #[error("Coefficient must be non-zero")]
    Zero,
    #[error("Coefficient must be finite")]
    Infinite,
    #[error("Coefficient must be not NaN")]
    NaN,
}

/// Coefficient of polynomial terms.
///
/// Invariants
/// -----------
/// - The value is not zero and finite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Coefficient(NotNan<f64>);

impl Coefficient {
    pub fn into_inner(self) -> f64 {
        self.0.into_inner()
    }

    /// ABS of the coefficient is also a coefficient.
    pub fn abs(&self) -> Self {
        Self(self.0.abs().try_into().unwrap())
    }
}

impl TryFrom<f64> for Coefficient {
    type Error = CoefficientError;
    fn try_from(value: f64) -> Result<Self, Self::Error> {
        if value.is_nan() {
            return Err(CoefficientError::NaN);
        }
        if !value.is_finite() {
            return Err(CoefficientError::Infinite);
        }
        if value == 0.0 {
            return Err(CoefficientError::Zero);
        }
        Ok(Self(NotNan::new(value).unwrap())) // Safe because we checked the value is not NaN
    }
}

impl From<Coefficient> for f64 {
    fn from(coefficient: Coefficient) -> Self {
        coefficient.into_inner()
    }
}

impl Deref for Coefficient {
    type Target = f64;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Add for Coefficient {
    type Output = Option<Self>;
    fn add(self, rhs: Self) -> Self::Output {
        let sum = self.0 + rhs.0;
        // Check cancellation since Coefficient is not zero
        if sum == 0.0 {
            None
        } else {
            Some(Self(sum))
        }
    }
}

// Non-zero * Non-zero = Non-zero
impl Mul for Coefficient {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        Self(self.0 * rhs.0)
    }
}

impl MulAssign for Coefficient {
    fn mul_assign(&mut self, rhs: Self) {
        self.0 *= rhs.0;
    }
}

impl Neg for Coefficient {
    type Output = Self;
    fn neg(self) -> Self::Output {
        Self(-self.0)
    }
}

impl Sub for Coefficient {
    type Output = Option<Self>;
    fn sub(self, rhs: Self) -> Self::Output {
        self + (-rhs)
    }
}

impl Arbitrary for Coefficient {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;
    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        prop_oneof![Just(1.0), Just(-1.0), -10.0..10.0]
            .prop_filter("nonzero", |x: &f64| x.abs() > f64::EPSILON)
            .prop_map(|x| Coefficient::try_from(x).unwrap())
            .boxed()
    }
}

impl PartialEq<f64> for Coefficient {
    fn eq(&self, other: &f64) -> bool {
        if let Ok(other) = TryInto::<Coefficient>::try_into(*other) {
            *self == other
        } else {
            false
        }
    }
}

impl PartialOrd<f64> for Coefficient {
    fn partial_cmp(&self, other: &f64) -> Option<std::cmp::Ordering> {
        Some(self.into_inner().total_cmp(other))
    }
}
