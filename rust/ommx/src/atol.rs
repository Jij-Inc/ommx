use approx::AbsDiffEq;
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

/// Absolute tolerance for inclusive approximate comparisons.
///
/// [`Self::approx_eq`], [`Self::approx_is_zero`], and [`Self::approx_le`] are
/// the canonical scalar comparison primitives. They include the tolerance
/// boundary. Approximate equality rejects non-finite operands or differences;
/// approximate ordering preserves strict IEEE ordering and uses approximate
/// equality only at the boundary. Construction requires a positive, non-NaN
/// value; positive infinity is currently accepted, so callers that own a
/// finite-tolerance invariant must continue to validate it at that boundary.
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

    /// Compare two values using this absolute tolerance.
    ///
    /// For finite operands this delegates to [`AbsDiffEq`] for [`f64`], so the
    /// tolerance boundary is inclusive: values compare approximately equal
    /// when `abs(lhs - rhs) <= atol`.
    ///
    /// Comparisons involving [`f64::NAN`], an infinite operand, or a
    /// non-finite subtraction always return `false`, including two infinities
    /// with the same sign and comparisons made with an infinite tolerance.
    /// This boolean helper does not replace finite-operand validation:
    /// evaluation and parsing APIs that reject non-finite values still perform
    /// those checks before or independently of approximate comparison.
    ///
    /// [`ATol::new`] currently accepts positive infinity. This method does not
    /// change that separate type invariant: an infinite tolerance makes finite
    /// operands approximately equal whenever their subtraction remains finite.
    #[inline]
    pub fn approx_eq(self, lhs: f64, rhs: f64) -> bool {
        let difference = lhs - rhs;
        lhs.is_finite()
            && rhs.is_finite()
            && difference.is_finite()
            && lhs.abs_diff_eq(&rhs, self.into_inner())
    }

    /// Classify a value as approximately zero using this absolute tolerance.
    ///
    /// This is exactly `self.approx_eq(value, 0.0)`, including the tolerance
    /// boundary and the non-finite-value behavior documented by
    /// [`Self::approx_eq`].
    #[inline]
    pub fn approx_is_zero(self, value: f64) -> bool {
        self.approx_eq(value, 0.0)
    }

    /// Compare whether `lhs` is less than or approximately equal to `rhs`.
    ///
    /// This is the shared one-sided feasibility predicate for a residual
    /// constrained by `lhs - rhs <= 0`. It preserves exact strict ordering and
    /// includes the absolute-tolerance boundary through [`Self::approx_eq`]:
    /// `lhs < rhs || self.approx_eq(lhs, rhs)`.
    ///
    /// Consequently, NaN never satisfies this comparison. Strictly ordered
    /// infinities retain their IEEE ordering, while equal infinities are not
    /// approximately equal. APIs that require finite evaluated values continue
    /// to validate that invariant separately.
    #[inline]
    pub fn approx_le(self, lhs: f64, rhs: f64) -> bool {
        lhs < rhs || self.approx_eq(lhs, rhs)
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

    #[test]
    fn approximate_comparison_includes_the_boundary() {
        let atol = ATol::new(0.125).unwrap();

        assert!(atol.approx_eq(1.0, 1.125));
        assert!(atol.approx_eq(1.125, 1.0));
        assert!(!atol.approx_eq(1.0, 1.1250000000000002));

        assert!(atol.approx_is_zero(0.125));
        assert!(atol.approx_is_zero(-0.125));
        assert!(!atol.approx_is_zero(0.12500000000000003));

        assert!(atol.approx_le(1.125, 1.0));
        assert!(atol.approx_le(0.5, 1.0));
        assert!(!atol.approx_le(1.1250000000000002, 1.0));
    }

    #[test]
    fn approximate_comparison_does_not_accept_non_finite_operands() {
        let atol = ATol::new(0.125).unwrap();
        let infinite_atol = ATol::new(f64::INFINITY).unwrap();

        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(!atol.approx_eq(value, value));
            assert!(!atol.approx_eq(value, 0.0));
            assert!(!atol.approx_eq(0.0, value));
            assert!(!atol.approx_is_zero(value));
            assert!(!atol.approx_le(value, value));
            assert!(!infinite_atol.approx_eq(value, value));
            assert!(!infinite_atol.approx_is_zero(value));
        }

        assert!(atol.approx_le(f64::NEG_INFINITY, 0.0));
        assert!(atol.approx_le(0.0, f64::INFINITY));
        assert!(!atol.approx_le(f64::INFINITY, 0.0));
        assert!(!atol.approx_le(0.0, f64::NEG_INFINITY));

        // The ATol finite-value invariant is intentionally a separate concern.
        assert!(infinite_atol.approx_eq(1.0, 2.0));
        assert!(!infinite_atol.approx_eq(f64::MAX, -f64::MAX));
    }
}
