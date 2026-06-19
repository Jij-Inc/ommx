use num::traits::Inv;
use ordered_float::NotNan;
use proptest::prelude::*;
use std::{
    fmt::{Debug, Display},
    ops::{Add, Deref, Div, Mul, Neg, Sub},
};

use crate::ATol;

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CoefficientError {
    #[error("Coefficient must be non-zero")]
    Zero,
    #[error("Coefficient must be finite")]
    Infinite,
    #[error("Coefficient must be not NaN")]
    NaN,
}

/// Result of coefficient arithmetic.
///
/// - `Ok(Some(coefficient))`: the result is a finite non-zero coefficient.
/// - `Ok(None)`: the result is exactly zero, so the owning polynomial term should be removed.
/// - `Err(error)`: the result is NaN or infinite and cannot be represented as a coefficient.
pub type CoefficientArithmeticResult = Result<Option<Coefficient>, CoefficientError>;

/// Coefficient of polynomial terms.
///
/// The value is always finite, non-zero, and not NaN. Arithmetic returns
/// [`CoefficientArithmeticResult`] because cancellation or underflow can remove
/// a term, while overflow and NaN are invalid coefficient values.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[repr(transparent)]
pub struct Coefficient(NotNan<f64>);

impl<'de> serde::Deserialize<'de> for Coefficient {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = f64::deserialize(deserializer)?;
        Coefficient::try_from(value).map_err(serde::de::Error::custom)
    }
}

impl Debug for Coefficient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Coefficient({})", self.0)
    }
}

impl Display for Coefficient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Coefficient {
    pub fn one() -> Self {
        Coefficient(NotNan::new(1.0).unwrap())
    }

    pub fn into_inner(self) -> f64 {
        self.0.into_inner()
    }

    /// ABS of the coefficient.
    pub fn abs(&self) -> Self {
        Self(self.0.abs().try_into().unwrap())
    }

    fn classify_arithmetic(value: f64) -> CoefficientArithmeticResult {
        match Coefficient::try_from(value) {
            Ok(coefficient) => Ok(Some(coefficient)),
            Err(CoefficientError::Zero) => Ok(None),
            Err(error) => Err(error),
        }
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
    type Output = CoefficientArithmeticResult;
    fn add(self, rhs: Self) -> Self::Output {
        Self::classify_arithmetic(self.into_inner() + rhs.into_inner())
    }
}

impl Mul for Coefficient {
    type Output = CoefficientArithmeticResult;
    fn mul(self, rhs: Self) -> Self::Output {
        Self::classify_arithmetic(self.into_inner() * rhs.into_inner())
    }
}

impl Div for Coefficient {
    type Output = CoefficientArithmeticResult;
    fn div(self, rhs: Self) -> Self::Output {
        Self::classify_arithmetic(self.into_inner() / rhs.into_inner())
    }
}

impl Neg for Coefficient {
    type Output = Self;
    fn neg(self) -> Self::Output {
        Self(-self.0)
    }
}

impl Sub for Coefficient {
    type Output = CoefficientArithmeticResult;
    fn sub(self, rhs: Self) -> Self::Output {
        self + (-rhs)
    }
}

impl Inv for Coefficient {
    type Output = Result<Self, CoefficientError>;
    fn inv(self) -> Self::Output {
        Coefficient::try_from(self.into_inner().recip())
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

impl PartialEq<ATol> for Coefficient {
    fn eq(&self, other: &ATol) -> bool {
        self.into_inner() == other.into_inner()
    }
}

impl PartialOrd<ATol> for Coefficient {
    fn partial_cmp(&self, other: &ATol) -> Option<std::cmp::Ordering> {
        self.into_inner().partial_cmp(&other.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coeff;

    fn tiny() -> Coefficient {
        Coefficient::try_from(f64::from_bits(1)).unwrap()
    }

    fn huge() -> Coefficient {
        Coefficient::try_from(f64::MAX).unwrap()
    }

    fn unwrap_some(result: CoefficientArithmeticResult) -> Coefficient {
        result.unwrap().unwrap()
    }

    #[test]
    fn arithmetic_can_remove_terms() {
        assert_eq!(coeff!(1.0) + coeff!(-1.0), Ok(None));

        let tiny = Coefficient::try_from(f64::from_bits(1)).unwrap();
        assert_eq!(tiny * tiny, Ok(None));
        assert_eq!(tiny / huge(), Ok(None));
    }

    #[test]
    fn arithmetic_rejects_non_finite_results() {
        assert!(matches!(huge() + huge(), Err(CoefficientError::Infinite)));
        assert!(matches!(huge() * huge(), Err(CoefficientError::Infinite)));
        assert!(matches!(tiny().inv(), Err(CoefficientError::Infinite)));
    }

    #[test]
    fn arithmetic_preserves_finite_nonzero_results() {
        assert_eq!(unwrap_some(coeff!(2.0) + coeff!(3.0)), coeff!(5.0));
        assert_eq!(unwrap_some(coeff!(2.0) - coeff!(3.0)), coeff!(-1.0));
        assert_eq!(unwrap_some(coeff!(2.0) * coeff!(3.0)), coeff!(6.0));
        assert_eq!(unwrap_some(coeff!(6.0) / coeff!(3.0)), coeff!(2.0));
        assert_eq!(coeff!(2.0).inv().unwrap(), coeff!(0.5));
    }

    #[test]
    fn deserialize_rejects_invalid_values() {
        assert!(serde_json::from_str::<Coefficient>("1.0").is_ok());
        assert!(serde_json::from_str::<Coefficient>("0.0").is_err());
        assert!(serde_json::from_str::<Coefficient>("1e999").is_err());
        assert!(serde_json::from_str::<Coefficient>("NaN").is_err());
    }
}
