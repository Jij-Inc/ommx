use crate::{
    macros::{impl_add_inverse, impl_mul_inverse},
    parse::{Parse, ParseError, RawParseError},
    v1, VariableID,
};
use num::Zero;
use proptest::prelude::*;
use std::{collections::HashMap, ops::*};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BoundError {
    #[error("lower({lower}) or upper({upper}) never be NAN")]
    NotANumber { lower: f64, upper: f64 },
    #[error("lower({lower}) = +inf or upper({upper}) = -inf is not allowed")]
    InvalidInfinity { lower: f64, upper: f64 },
    #[error("Lower is larger than Upper: lower({lower}) > upper({upper})")]
    UpperSmallerThanLower { lower: f64, upper: f64 },
}

impl BoundError {
    fn check(lower: f64, upper: f64) -> Result<(), BoundError> {
        if lower.is_nan() || upper.is_nan() {
            return Err(BoundError::NotANumber { lower, upper });
        }
        if lower == f64::INFINITY || upper == f64::NEG_INFINITY {
            return Err(BoundError::InvalidInfinity { lower, upper });
        }
        if lower > upper {
            return Err(BoundError::UpperSmallerThanLower { lower, upper });
        }
        Ok(())
    }
}

impl From<BoundError> for ParseError {
    fn from(e: BoundError) -> Self {
        RawParseError::from(e).into()
    }
}

pub type Bounds = HashMap<VariableID, Bound>;

/// Bound of a decision variable
///
/// Invariant
/// ---------
/// - `lower <= upper`
/// - `lower` and `upper` never become `NaN`
/// - `lower` is not `+inf` and `upper` is not `-inf`
///
/// Examples
/// --------
///
/// ```rust
/// use ommx::Bound;
///
/// // Usual
/// let bound = Bound::try_from([1.0, 2.0]).unwrap();
/// // Single point `[1.0, 1.0]`
/// let bound = Bound::try_from(1.0).unwrap();
/// // Default is `(-inf, inf)`
/// assert_eq!(Bound::default(), Bound::try_from([f64::NEG_INFINITY, f64::INFINITY]).unwrap());
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bound {
    lower: f64,
    upper: f64,
}

impl Default for Bound {
    fn default() -> Self {
        Self {
            lower: f64::NEG_INFINITY,
            upper: f64::INFINITY,
        }
    }
}

impl Parse for v1::Bound {
    type Output = Bound;
    type Context = ();
    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let out = Bound::new(self.lower, self.upper)?;
        Ok(out)
    }
}

impl TryFrom<v1::Bound> for Bound {
    type Error = BoundError;
    fn try_from(value: v1::Bound) -> Result<Self, Self::Error> {
        Self::new(value.lower, value.upper)
    }
}

impl TryFrom<f64> for Bound {
    type Error = BoundError;
    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Self::new(value, value)
    }
}

impl TryFrom<[f64; 2]> for Bound {
    type Error = BoundError;
    fn try_from([lower, upper]: [f64; 2]) -> Result<Self, Self::Error> {
        Self::new(lower, upper)
    }
}

impl Add for Bound {
    type Output = Bound;
    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.lower + rhs.lower, self.upper + rhs.upper).unwrap()
    }
}
impl Add<f64> for Bound {
    type Output = Bound;
    fn add(self, rhs: f64) -> Self::Output {
        Bound::new(self.lower + rhs, self.upper + rhs).unwrap()
    }
}
impl_add_inverse!(f64, Bound);

impl AddAssign for Bound {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl AddAssign<f64> for Bound {
    fn add_assign(&mut self, rhs: f64) {
        *self = *self + rhs;
    }
}

impl Zero for Bound {
    fn zero() -> Self {
        Self::try_from(0.0).unwrap()
    }
    fn is_zero(&self) -> bool {
        self.lower == 0.0 && self.upper == 0.0
    }
}

impl Mul for Bound {
    type Output = Bound;
    fn mul(self, rhs: Self) -> Self::Output {
        // [0, 0] x (-inf, inf) = [0, 0]
        if self == Bound::zero() || rhs == Bound::zero() {
            return Bound::zero();
        }
        let a = self.lower * rhs.lower;
        let b = self.lower * rhs.upper;
        let c = self.upper * rhs.lower;
        let d = self.upper * rhs.upper;
        Bound::new(a.min(b).min(c).min(d), a.max(b).max(c).max(d)).unwrap()
    }
}

impl Mul<f64> for Bound {
    type Output = Bound;
    fn mul(self, rhs: f64) -> Self::Output {
        if rhs >= 0.0 {
            Bound::new(self.lower * rhs, self.upper * rhs).unwrap()
        } else {
            Bound::new(self.upper * rhs, self.lower * rhs).unwrap()
        }
    }
}
impl_mul_inverse!(f64, Bound);

impl MulAssign for Bound {
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}
impl MulAssign<f64> for Bound {
    fn mul_assign(&mut self, rhs: f64) {
        *self = *self * rhs;
    }
}

impl Bound {
    pub fn new(lower: f64, upper: f64) -> Result<Self, BoundError> {
        BoundError::check(lower, upper)?;
        Ok(Self { lower, upper })
    }

    pub fn lower(&self) -> f64 {
        self.lower
    }

    pub fn upper(&self) -> f64 {
        self.upper
    }

    pub fn width(&self) -> f64 {
        self.upper - self.lower
    }

    pub fn set_lower(&mut self, lower: f64) -> Result<(), BoundError> {
        BoundError::check(lower, self.upper)?;
        self.lower = lower;
        Ok(())
    }

    pub fn set_upper(&mut self, upper: f64) -> Result<(), BoundError> {
        BoundError::check(self.lower, upper)?;
        self.upper = upper;
        Ok(())
    }

    /// Strengthen the bound for integer decision variables
    ///
    /// Since the bound evaluation may be inaccurate due to floating-point arithmetic error,
    /// this method rounds to `[ceil(lower-atol), floor(upper+atol)]` with `atol = 1e-6`.
    pub fn as_integer_bound(&self) -> Self {
        let atol = 1e-6;
        let lower = if self.lower.is_finite() {
            (self.lower - atol).ceil()
        } else {
            self.lower
        };
        let upper = if self.upper.is_finite() {
            (self.upper + atol).floor()
        } else {
            self.upper
        };
        Self::new(lower, upper).unwrap()
    }

    /// `[lower, upper]` with finite `lower` and `upper`
    pub fn is_finite(&self) -> bool {
        self.lower.is_finite() && self.upper.is_finite()
    }

    pub fn pow(&self, exp: u8) -> Self {
        if exp % 2 == 0 {
            if self.lower >= 0.0 {
                // 0 <= lower <= upper
                Bound::new(self.lower.powi(exp as i32), self.upper.powi(exp as i32)).unwrap()
            } else if self.upper <= 0.0 {
                // lower <= upper <= 0
                Bound::new(self.upper.powi(exp as i32), self.lower.powi(exp as i32)).unwrap()
            } else {
                // lower <= 0 <= upper
                Bound::new(
                    0.0,
                    self.upper
                        .abs()
                        .powi(exp as i32)
                        .max(self.lower.abs().powi(exp as i32)),
                )
                .unwrap()
            }
        } else {
            // pow is monotonic for odd exponents
            Bound::new(self.lower.powi(exp as i32), self.upper.powi(exp as i32)).unwrap()
        }
    }

    /// Check the `value` is in the bound with absolute tolerance
    pub fn contains(&self, value: f64, atol: f64) -> bool {
        self.lower - atol <= value && value <= self.upper + atol
    }

    pub fn as_range(&self) -> RangeInclusive<f64> {
        self.lower..=self.upper
    }

    /// Arbitrary *finite* value within the bound
    ///
    /// `max_abs` is the maximum absolute value of the generated value
    /// to keep floating point arithmetic stable.
    pub fn arbitrary_containing(&self, max_abs: f64) -> BoxedStrategy<f64> {
        assert!(max_abs > 0.0);
        // RangeInclusive::arbitrary() does not support infinite range
        match (self.lower, self.upper) {
            (f64::NEG_INFINITY, f64::INFINITY) => (-max_abs..=max_abs).boxed(),
            (f64::NEG_INFINITY, upper) => {
                let upper = upper.min(max_abs);
                prop_oneof![(-max_abs..=upper).boxed(), Just(upper)].boxed()
            }
            (lower, f64::INFINITY) => {
                let lower = lower.max(-max_abs);
                prop_oneof![(lower..=max_abs).boxed(), Just(lower)].boxed()
            }
            (lower, upper) => {
                let lower = lower.max(-max_abs);
                let upper = upper.min(max_abs);
                if lower == upper {
                    Just(lower).boxed()
                } else {
                    prop_oneof![Just(upper), Just(lower), (lower..=upper)].boxed()
                }
            }
        }
    }
}

impl Arbitrary for Bound {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;
    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        (
            prop_oneof![
                Just(f64::NEG_INFINITY),
                Just(0.0),
                (-10..=10).prop_map(|x| x as f64),
                -10.0..=10.0
            ],
            prop_oneof![
                Just(f64::INFINITY),
                Just(0.0),
                (-10..=10).prop_map(|x| x as f64),
                -10.0..=10.0
            ],
        )
            .prop_map(|(lower, upper)| {
                if lower <= upper {
                    Bound::new(lower, upper).unwrap()
                } else {
                    Bound::new(upper, lower).unwrap()
                }
            })
            .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bound_pow() {
        insta::assert_debug_snapshot!(Bound::new(2.0, 3.0).unwrap().pow(2), @r###"
        Bound {
            lower: 4.0,
            upper: 9.0,
        }
        "###);
        insta::assert_debug_snapshot!(Bound::new(2.0, 3.0).unwrap().pow(3), @r###"
        Bound {
            lower: 8.0,
            upper: 27.0,
        }
        "###);
        insta::assert_debug_snapshot!(Bound::new(-2.0, 3.0).unwrap().pow(2), @r###"
        Bound {
            lower: 0.0,
            upper: 9.0,
        }
        "###);
        insta::assert_debug_snapshot!(Bound::new(-2.0, 3.0).unwrap().pow(3), @r###"
        Bound {
            lower: -8.0,
            upper: 27.0,
        }
        "###);
        insta::assert_debug_snapshot!(Bound::default().pow(2), @r###"
        Bound {
            lower: 0.0,
            upper: inf,
        }
        "###);
        insta::assert_debug_snapshot!(Bound::default().pow(3), @r###"
        Bound {
            lower: -inf,
            upper: inf,
        }
        "###);
    }

    fn bound_and_containing() -> BoxedStrategy<(Bound, f64)> {
        Bound::arbitrary()
            .prop_flat_map(|bound| (Just(bound), bound.arbitrary_containing(1e5)))
            .boxed()
    }

    #[test]
    fn as_integer_bound() {
        assert_eq!(
            Bound::new(1.000000000001, 1.99999999999)
                .unwrap()
                .as_integer_bound(),
            Bound::new(1.0, 2.0).unwrap()
        )
    }

    proptest! {
        #[test]
        fn contains((bound, value) in bound_and_containing()) {
            prop_assert!(bound.contains(value, 1e-9));
        }

        #[test]
        fn add((b1, v1) in bound_and_containing(), (b2, v2) in bound_and_containing()) {
            prop_assert!((b1 + b2).contains(v1 + v2, 1e-9));
        }

        #[test]
        fn mul((b1, v1) in bound_and_containing(), (b2, v2) in bound_and_containing()) {
            prop_assert!((b1 * b2).contains(v1 * v2, 1e-9));
        }

        #[test]
        fn pow((b, v) in bound_and_containing()) {
            prop_assert!(b.pow(2).contains(v.powi(2), 1e-9));
            prop_assert!(b.pow(3).contains(v.powi(3), 1e-9));
        }
    }
}
