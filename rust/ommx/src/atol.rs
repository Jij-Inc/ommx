use ordered_float::{FloatIsNan, NotNan};
use std::ops::{Add, Deref, Neg, Sub};
use std::sync::{LazyLock, RwLock};

use crate::Coefficient;

/// Error produced when constructing an [`ATol`] or updating its default.
///
/// Signal-style typed error — callers may downcast from [`crate::Error`]
/// when they need to distinguish invalid inputs. See the
/// [`crate::error`](crate) module for the downcast convention.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AtolError {
    /// `value` was zero or negative. Absolute tolerances must be strictly
    /// positive.
    #[error("ATol must be positive: got {value}")]
    NonPositive { value: f64 },

    /// `value` was NaN.
    #[error("ATol cannot be NaN")]
    NaN,
}

impl From<FloatIsNan> for AtolError {
    fn from(_: FloatIsNan) -> Self {
        AtolError::NaN
    }
}

/// Absolute tolerance
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ATol(NotNan<f64>);

static DEFAULT_ATOL: LazyLock<RwLock<f64>> = LazyLock::new(|| {
    let default_value = match std::env::var("OMMX_DEFAULT_ATOL") {
        Ok(s) => {
            match s.parse::<f64>() {
                Ok(v) if v > 0.0 => {
                    tracing::info!("Using OMMX_DEFAULT_ATOL environment variable: {v}");
                    v
                }
                Ok(v) => {
                    tracing::warn!("Invalid OMMX_DEFAULT_ATOL value (must be positive): {v}. Using default 1e-6");
                    1e-6
                }
                Err(_) => {
                    tracing::warn!(
                        "Invalid OMMX_DEFAULT_ATOL value (not a number): '{s}'. Using default 1e-6"
                    );
                    1e-6
                }
            }
        }
        Err(_) => 1e-6,
    };

    RwLock::new(default_value)
});

impl Deref for ATol {
    type Target = f64;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ATol {
    pub fn new(value: f64) -> crate::Result<Self> {
        if value.is_nan() {
            tracing::error!(value, "ATol cannot be NaN");
            return Err(AtolError::NaN.into());
        }
        if value <= 0.0 {
            tracing::error!(value, "ATol must be positive");
            return Err(AtolError::NonPositive { value }.into());
        }
        Ok(ATol(NotNan::new(value).map_err(AtolError::from)?))
    }

    pub fn into_inner(&self) -> f64 {
        self.0.into_inner()
    }

    #[tracing::instrument(skip_all)]
    pub fn set_default(value: f64) -> crate::Result<()> {
        let atol = Self::new(value)?;
        let mut default = DEFAULT_ATOL.write().map_err(|e| {
            crate::error!("Failed to acquire write lock for DEFAULT_ATOL: poisoned lock: {e}")
        })?;
        *default = atol.into_inner();
        tracing::info!("ATol default value changed to: {value}");
        Ok(())
    }
}

impl PartialEq<f64> for ATol {
    fn eq(&self, other: &f64) -> bool {
        self.into_inner() == *other
    }
}

impl PartialOrd<f64> for ATol {
    fn partial_cmp(&self, other: &f64) -> Option<std::cmp::Ordering> {
        self.into_inner().partial_cmp(other)
    }
}

impl PartialEq<ATol> for f64 {
    fn eq(&self, other: &ATol) -> bool {
        *self == other.into_inner()
    }
}

impl PartialOrd<ATol> for f64 {
    fn partial_cmp(&self, other: &ATol) -> Option<std::cmp::Ordering> {
        self.partial_cmp(&other.into_inner())
    }
}

impl PartialEq<Coefficient> for ATol {
    fn eq(&self, other: &Coefficient) -> bool {
        self.into_inner() == other.into_inner()
    }
}

impl PartialOrd<Coefficient> for ATol {
    fn partial_cmp(&self, other: &Coefficient) -> Option<std::cmp::Ordering> {
        self.into_inner().partial_cmp(&other.into_inner())
    }
}

impl Default for ATol {
    fn default() -> Self {
        let default_value = match DEFAULT_ATOL.read() {
            Ok(guard) => *guard,
            Err(_) => 1e-6,
        };
        ATol(NotNan::new(default_value).unwrap())
    }
}

impl Add<f64> for ATol {
    type Output = f64;
    fn add(self, rhs: f64) -> Self::Output {
        self.into_inner() + rhs
    }
}

impl Add<ATol> for f64 {
    type Output = f64;
    fn add(self, rhs: ATol) -> Self::Output {
        self + rhs.into_inner()
    }
}

impl Sub<f64> for ATol {
    type Output = f64;
    fn sub(self, rhs: f64) -> Self::Output {
        self.into_inner() - rhs
    }
}

impl Sub<ATol> for f64 {
    type Output = f64;
    fn sub(self, rhs: ATol) -> Self::Output {
        self - rhs.into_inner()
    }
}

impl Neg for ATol {
    type Output = f64;
    fn neg(self) -> Self::Output {
        -self.into_inner()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_positive() {
        let err = ATol::new(0.0).unwrap_err();
        assert!(err.is::<AtolError>());
        match err.downcast::<AtolError>() {
            Ok(AtolError::NonPositive { value }) => assert_eq!(value, 0.0),
            Ok(other) => panic!("unexpected variant: {other:?}"),
            Err(_) => panic!("downcast failed"),
        }
    }

    #[test]
    fn rejects_negative() {
        let err = ATol::new(-1e-9).unwrap_err();
        assert!(err.is::<AtolError>());
    }

    #[test]
    fn rejects_nan() {
        let err = ATol::new(f64::NAN).unwrap_err();
        match err.downcast::<AtolError>() {
            Ok(AtolError::NaN) => {}
            Ok(other) => panic!("unexpected variant: {other:?}"),
            Err(_) => panic!("downcast failed"),
        }
    }

    #[test]
    fn accepts_small_positive() {
        assert!(ATol::new(1e-12).is_ok());
    }
}
