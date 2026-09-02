use crate::{
    macros::{impl_add_inverse, impl_mul_inverse},
    parse::{Parse, ParseError},
    v1, ATol, VariableID,
};
use approx::AbsDiffEq;
use num::Zero;
use proptest::prelude::*;
use std::{collections::BTreeMap, ops::*};
use thiserror::Error;

#[non_exhaustive]
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
        ParseError::new(e)
    }
}

/// Bound for each decision variable
///
/// This uses `BTreeMap` to keep the order of decision variables by their IDs
/// for intuitive debugging.
pub type Bounds = BTreeMap<VariableID, Bound>;

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
#[derive(
    Clone,
    Copy,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    crate::logical_memory::LogicalMemoryProfile,
)]
pub struct Bound {
    lower: f64,
    upper: f64,
}

impl std::fmt::Debug for Bound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Bound{self}")
    }
}

impl std::fmt::Display for Bound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let lower_bracket = if self.lower == f64::NEG_INFINITY {
            "("
        } else {
            "["
        };
        let upper_bracket = if self.upper == f64::INFINITY {
            ")"
        } else {
            "]"
        };

        let lower_str = if self.lower == f64::NEG_INFINITY {
            "-inf".to_string()
        } else {
            self.lower.to_string()
        };

        let upper_str = if self.upper == f64::INFINITY {
            "inf".to_string()
        } else {
            self.upper.to_string()
        };

        write!(f, "{lower_bracket}{lower_str}, {upper_str}{upper_bracket}")
    }
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

impl TryFrom<&v1::DecisionVariable> for Bound {
    type Error = BoundError;
    fn try_from(v: &v1::DecisionVariable) -> Result<Self, Self::Error> {
        if let Some(bound) = &v.bound {
            Self::try_from(bound.clone())
        } else if v.kind() == v1::decision_variable::Kind::Binary {
            Self::new(0.0, 1.0)
        } else {
            Ok(Self::default())
        }
    }
}

impl From<Bound> for v1::Bound {
    fn from(bound: Bound) -> Self {
        Self {
            lower: bound.lower,
            upper: bound.upper,
        }
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

impl PartialEq<f64> for Bound {
    fn eq(&self, other: &f64) -> bool {
        self.lower == *other && self.upper == *other
    }
}

impl PartialEq<Bound> for f64 {
    fn eq(&self, other: &Bound) -> bool {
        other == self
    }
}

/// - `a <= [b, c]` means `a <= b`, i.e. `a <= x (forall x \in [b, c])`
/// - `a >= [b, c]` means `a >= c`, i.e. `a >= x (forall x \in [b, c])`
/// - If `a` is in `[b, c]`, return `None`
impl PartialOrd<f64> for Bound {
    fn partial_cmp(&self, other: &f64) -> Option<std::cmp::Ordering> {
        debug_assert!(
            self.lower <= self.upper,
            "lower({}) <= upper({})",
            self.lower,
            self.upper
        );
        if other <= &self.lower {
            Some(std::cmp::Ordering::Greater)
        } else if other >= &self.upper {
            Some(std::cmp::Ordering::Less)
        } else {
            None
        }
    }
}

impl PartialOrd<Bound> for f64 {
    fn partial_cmp(&self, other: &Bound) -> Option<std::cmp::Ordering> {
        other.partial_cmp(self).map(|o| o.reverse())
    }
}

impl AbsDiffEq for Bound {
    type Epsilon = ATol;

    fn default_epsilon() -> Self::Epsilon {
        ATol::default()
    }

    fn abs_diff_eq(&self, other: &Self, epsilon: Self::Epsilon) -> bool {
        // Since `abs_diff_eq` for f64::INFINITY returns false, check it first
        (self.lower == other.lower || epsilon.approx_eq(self.lower, other.lower))
            && (self.upper == other.upper || epsilon.approx_eq(self.upper, other.upper))
    }
}

impl Bound {
    /// Positive or zero, `[0, inf)`
    pub fn positive() -> Self {
        Self::new(0.0, f64::INFINITY).unwrap()
    }

    /// Negative or zero, `(-inf, 0]`
    pub fn negative() -> Self {
        Self::new(f64::NEG_INFINITY, 0.0).unwrap()
    }

    /// Unbounded, `(-inf, inf)`, which equals to `Bound::default()`
    pub fn unbounded() -> Self {
        Self::default()
    }

    pub fn of_binary() -> Self {
        Self::new(0.0, 1.0).unwrap()
    }

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

    /// Strengthen the bound for integer decision variables.
    ///
    /// Each finite returned endpoint is the smallest or largest integer value
    /// satisfying this bound under the same residual-feasibility semantics as
    /// [`Self::contains`]; an unbounded side remains unbounded. If no integer
    /// value satisfies the bound, this returns `None`.
    ///
    /// Examples
    /// ---------
    ///
    /// ```rust
    /// use ommx::{Bound, BoundError, ATol};
    ///
    /// // Rounding with absolute tolerance
    /// let bound = Bound::new(1.000000000001, 1.99999999999).unwrap();
    /// assert_eq!(bound.as_integer_bound(ATol::default()).unwrap(), Bound::new(1.0, 2.0).unwrap());
    ///
    /// // No integer value exists between 1.1 and 1.9
    /// let bound = Bound::new(1.1, 1.9).unwrap();
    /// assert!(bound.as_integer_bound(ATol::default()).is_none());
    ///
    /// // infinite bound are kept as is
    /// let bound = Bound::new(f64::NEG_INFINITY, f64::INFINITY).unwrap();
    /// assert_eq!(bound.as_integer_bound(ATol::default()), Some(bound));
    ///
    /// let bound = Bound::new(1.1, f64::INFINITY).unwrap();
    /// assert_eq!(bound.as_integer_bound(ATol::default()).unwrap(), Bound::new(2.0, f64::INFINITY).unwrap());
    /// ```
    pub fn as_integer_bound(&self, atol: crate::ATol) -> Option<Self> {
        if !self.lower.is_finite() && !self.upper.is_finite() {
            return Some(*self);
        }
        let (finite_lower, finite_upper) = self.finite_feasible_extrema(atol);
        let lower = if self.lower.is_finite() {
            let out = finite_lower.ceil();
            if out == 0.0 {
                0.0 // Avoid negative zero
            } else {
                out
            }
        } else {
            self.lower
        };
        let upper = if self.upper.is_finite() {
            finite_upper.floor()
        } else {
            self.upper
        };
        if upper < lower {
            None
        } else {
            Some(Self { lower, upper })
        }
    }

    /// Check if the bound is a point, i.e. `lower == upper`
    pub fn is_point(&self, atol: ATol) -> Option<f64> {
        if atol.approx_eq(self.lower, self.upper) {
            Some(self.lower)
        } else {
            None
        }
    }

    /// `[lower, upper]` with finite `lower` and `upper`
    pub fn is_finite(&self) -> bool {
        self.lower.is_finite() && self.upper.is_finite()
    }

    /// Take the intersection of two bounds, `None` if the intersection is empty
    pub fn intersection(&self, other: &Self) -> Option<Self> {
        Self::new(self.lower.max(other.lower), self.upper.min(other.upper)).ok()
    }

    pub fn pow(&self, exp: u8) -> Self {
        if exp.is_multiple_of(2) {
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

    /// Check whether `value` is in the bound with absolute tolerance.
    ///
    /// A bound is interpreted as the conjunction of the residual constraints
    /// `lower - value <= 0` and `value - upper <= 0`. Both residuals use
    /// [`ATol::approx_le`], exactly like a less-than-or-equal-to-zero regular
    /// constraint after finite evaluation. An infinite endpoint means that its
    /// side is unconstrained; non-finite values and overflow while evaluating a
    /// finite-endpoint residual are rejected.
    pub fn contains(&self, value: f64, atol: crate::ATol) -> bool {
        if !value.is_finite() {
            return false;
        }

        let lower_satisfied = if self.lower == f64::NEG_INFINITY {
            true
        } else {
            let residual = self.lower - value;
            residual.is_finite() && atol.approx_le(residual, 0.0)
        };
        let upper_satisfied = if self.upper == f64::INFINITY {
            true
        } else {
            let residual = value - self.upper;
            residual.is_finite() && atol.approx_le(residual, 0.0)
        };
        lower_satisfied && upper_satisfied
    }

    /// Return the least and greatest finite `f64` values accepted by this
    /// bound under `atol`.
    ///
    /// Membership is monotone on each side of an exactly feasible seed. A
    /// binary search over the total order of finite `f64` values therefore
    /// derives the actual representable extrema without materializing
    /// `lower - atol` or `upper + atol`. This remains crate-private because the
    /// endpoints are an implementation tool for domain-owned normalization and
    /// validation, not a new mathematical Bound representation.
    pub(crate) fn finite_feasible_extrema(&self, atol: ATol) -> (f64, f64) {
        const SIGN_MASK: u64 = 1_u64 << 63;

        fn ordered_key(value: f64) -> u64 {
            debug_assert!(value.is_finite());
            let bits = value.to_bits();
            if bits & SIGN_MASK == 0 {
                bits | SIGN_MASK
            } else {
                !bits
            }
        }

        fn from_ordered_key(key: u64) -> f64 {
            let bits = if key & SIGN_MASK == 0 {
                !key
            } else {
                key & !SIGN_MASK
            };
            let value = f64::from_bits(bits);
            debug_assert!(value.is_finite());
            value
        }

        let seed = self.nearest_to_zero();
        debug_assert!(seed.is_finite());
        debug_assert!(self.contains(seed, atol));

        let mut lower_false_or_first = ordered_key(-f64::MAX);
        let mut lower_true = ordered_key(seed);
        while lower_false_or_first < lower_true {
            let middle = lower_false_or_first + (lower_true - lower_false_or_first) / 2;
            if self.contains(from_ordered_key(middle), atol) {
                lower_true = middle;
            } else {
                lower_false_or_first = middle + 1;
            }
        }

        let mut upper_true = ordered_key(seed);
        let mut upper_false_or_last = ordered_key(f64::MAX);
        while upper_true < upper_false_or_last {
            let distance = upper_false_or_last - upper_true;
            let middle = upper_true + distance / 2 + distance % 2;
            if self.contains(from_ordered_key(middle), atol) {
                upper_true = middle;
            } else {
                upper_false_or_last = middle - 1;
            }
        }

        (from_ordered_key(lower_true), from_ordered_key(upper_true))
    }

    pub fn as_range(&self) -> RangeInclusive<f64> {
        self.lower..=self.upper
    }

    pub fn nearest_to_zero(&self) -> f64 {
        if self.lower >= 0.0 {
            self.lower
        } else if self.upper <= 0.0 {
            self.upper
        } else {
            0.0
        }
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

    pub fn arbitrary_containing_integer(&self, max_abs: u64) -> BoxedStrategy<i64> {
        let lower = self.lower.max(-(max_abs as f64)).ceil() as i64;
        let upper = self.upper.min(max_abs as f64).floor() as i64;
        if lower == upper {
            Just(lower).boxed()
        } else {
            (lower..=upper).boxed()
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
    use std::error::Error as _;

    use approx::assert_abs_diff_eq;

    use super::*;

    #[test]
    fn parse_preserves_bound_error() {
        let error = v1::Bound {
            lower: 2.0,
            upper: 1.0,
        }
        .parse(&())
        .unwrap_err();

        assert!(matches!(
            error
                .source()
                .and_then(|source| source.downcast_ref::<BoundError>()),
            Some(BoundError::UpperSmallerThanLower { lower, upper })
                if *lower == 2.0 && *upper == 1.0
        ));
    }

    #[test]
    fn partial_ord() {
        assert!(1.0 <= Bound::new(2.0, 3.0).unwrap());
        assert!(2.0 <= Bound::new(2.0, 3.0).unwrap());
        assert!(3.0 >= Bound::new(2.0, 3.0).unwrap());
        assert!(4.0 >= Bound::new(2.0, 3.0).unwrap());
        assert!(f64::NEG_INFINITY <= Bound::new(2.0, 3.0).unwrap());
        assert!(f64::INFINITY >= Bound::new(2.0, 3.0).unwrap());
    }

    #[test]
    fn eq() {
        assert_eq!(
            Bound::new(f64::NEG_INFINITY, f64::INFINITY).unwrap(),
            Bound::default()
        );
        assert_eq!(Bound::new(0.0, f64::INFINITY).unwrap(), Bound::positive());
        assert_eq!(
            Bound::new(f64::NEG_INFINITY, 0.0).unwrap(),
            Bound::negative()
        );

        assert_abs_diff_eq!(
            Bound::new(1.0, 2.0).unwrap(),
            Bound::new(1.0, 2.00000001).unwrap(),
        );
        assert_abs_diff_eq!(
            Bound::new(f64::NEG_INFINITY, f64::INFINITY).unwrap(),
            Bound::default()
        );
        assert_abs_diff_eq!(Bound::new(0.0, f64::INFINITY).unwrap(), Bound::positive());
        assert_abs_diff_eq!(
            Bound::new(f64::NEG_INFINITY, 0.0).unwrap(),
            Bound::negative()
        );
    }

    #[test]
    fn intersection() {
        assert_eq!(
            Bound::new(1.0, 2.0)
                .unwrap()
                .intersection(&Bound::new(1.5, 3.0).unwrap()),
            Some(Bound::new(1.5, 2.0).unwrap())
        );
        assert_eq!(
            Bound::new(1.0, 2.0)
                .unwrap()
                .intersection(&Bound::new(2.5, 3.0).unwrap()),
            None
        );
        assert_eq!(
            Bound::positive().intersection(&Bound::negative()).unwrap(),
            0.0
        );
    }

    #[test]
    fn minus_zero() {
        let bound = Bound::new(-0.1, 1.1).unwrap();
        insta::assert_snapshot!(bound.as_integer_bound(ATol::default()).unwrap(), @"[0, 1]");
    }

    #[test]
    fn contains_uses_constraint_residual_semantics() {
        let atol = ATol::new(1.0e-6).unwrap();

        let upper = 2.0 / 3.0;
        let above = upper + atol.into_inner();
        let upper_bound = Bound::new(f64::NEG_INFINITY, upper).unwrap();
        let upper_residual = above - upper;
        assert_eq!(
            upper_bound.contains(above, atol),
            crate::Equality::LessThanOrEqualToZero.is_satisfied(upper_residual, atol)
        );
        assert!(!upper_bound.contains(above, atol));

        let below = 2.0;
        let lower = below + atol.into_inner();
        let lower_bound = Bound::new(lower, f64::INFINITY).unwrap();
        let lower_residual = lower - below;
        assert_eq!(
            lower_bound.contains(below, atol),
            crate::Equality::LessThanOrEqualToZero.is_satisfied(lower_residual, atol)
        );
        assert!(!lower_bound.contains(below, atol));

        assert!(!Bound::unbounded().contains(f64::NAN, atol));
        assert!(!Bound::unbounded().contains(f64::INFINITY, atol));
        assert!(!Bound::unbounded().contains(f64::NEG_INFINITY, atol));
    }

    #[test]
    fn contains_treats_infinite_endpoints_as_absent_constraints() {
        let atol = ATol::default();

        assert!(Bound::unbounded().contains(-f64::MAX, atol));
        assert!(Bound::unbounded().contains(f64::MAX, atol));
        assert!(Bound::negative().contains(-f64::MAX, atol));
        assert!(Bound::positive().contains(f64::MAX, atol));
    }

    #[test]
    fn contains_rejects_finite_endpoint_residual_overflow() {
        let atol = ATol::default();
        let bound = Bound::new(-f64::MAX, f64::MAX).unwrap();

        assert!(bound.contains(0.0, atol));
        assert!(!bound.contains(-f64::MAX, atol));
        assert!(!bound.contains(f64::MAX, atol));

        let (lower, upper) = bound.finite_feasible_extrema(atol);
        assert!(bound.contains(lower, atol));
        assert!(bound.contains(upper, atol));
        assert!(!bound.contains(lower.next_down(), atol));
        assert!(!bound.contains(upper.next_up(), atol));
    }

    #[test]
    fn finite_feasible_extrema_are_exactly_derived_from_contains() {
        let atol = ATol::new(1.0e-6).unwrap();
        let bound = Bound::new(-2.0, 2.0).unwrap();
        let (lower, upper) = bound.finite_feasible_extrema(atol);

        assert!(bound.contains(lower, atol));
        assert!(bound.contains(upper, atol));
        assert!(!bound.contains(lower.next_down(), atol));
        assert!(!bound.contains(upper.next_up(), atol));

        // Expanding the stored endpoint first does not necessarily produce a
        // value whose subsequently evaluated residual is within ATol.
        assert!(!bound.contains(2.0 + atol.into_inner(), atol));
        assert!(upper < 2.0 + atol.into_inner());
    }

    #[test]
    fn integer_bound_is_derived_from_bound_membership() {
        let atol = ATol::new(1.0e-6).unwrap();
        let bound = Bound::new(2.0 + atol.into_inner(), 4.0 - atol.into_inner()).unwrap();
        assert_eq!(
            bound.as_integer_bound(atol),
            Some(Bound::new(3.0, 3.0).unwrap())
        );
        assert!(!bound.contains(2.0, atol));
        assert!(!bound.contains(4.0, atol));

        assert_eq!(
            Bound::new(2.0 + atol.into_inner(), 3.0 - atol.into_inner())
                .unwrap()
                .as_integer_bound(atol),
            None
        );

        let large_atol = ATol::new(2.0).unwrap();
        assert_eq!(
            Bound::new(10.2, 10.3).unwrap().as_integer_bound(large_atol),
            Some(Bound::new(9.0, 12.0).unwrap())
        );
    }

    #[test]
    fn bound_pow() {
        insta::assert_debug_snapshot!(Bound::new(2.0, 3.0).unwrap().pow(2), @"Bound[4, 9]");
        insta::assert_debug_snapshot!(Bound::new(2.0, 3.0).unwrap().pow(3), @"Bound[8, 27]");
        insta::assert_debug_snapshot!(Bound::new(-2.0, 3.0).unwrap().pow(2), @"Bound[0, 9]");
        insta::assert_debug_snapshot!(Bound::new(-2.0, 3.0).unwrap().pow(3), @"Bound[-8, 27]");
        insta::assert_debug_snapshot!(Bound::default().pow(2), @"Bound[0, inf)");
        insta::assert_debug_snapshot!(Bound::default().pow(3), @"Bound(-inf, inf)");
    }

    fn bound_and_containing() -> BoxedStrategy<(Bound, f64)> {
        Bound::arbitrary()
            .prop_flat_map(|bound| (Just(bound), bound.arbitrary_containing(1e5)))
            .boxed()
    }

    proptest! {
        #[test]
        fn finite_feasible_extrema_are_membership_boundaries(
            bound in any::<Bound>(),
            tolerance in 1.0e-12_f64..10.0,
        ) {
            let atol = ATol::new(tolerance).unwrap();
            let (lower, upper) = bound.finite_feasible_extrema(atol);

            prop_assert!(lower <= upper);
            prop_assert!(bound.contains(lower, atol));
            prop_assert!(bound.contains(upper, atol));
            prop_assert!(!bound.contains(lower.next_down(), atol));
            prop_assert!(!bound.contains(upper.next_up(), atol));
        }

        #[test]
        fn contains((bound, value) in bound_and_containing()) {
            prop_assert!(bound.contains(value, crate::ATol::default()));
        }

        #[test]
        fn add((b1, v1) in bound_and_containing(), (b2, v2) in bound_and_containing()) {
            prop_assert!((b1 + b2).contains(v1 + v2, crate::ATol::default()));
        }

        #[test]
        fn mul((b1, v1) in bound_and_containing(), (b2, v2) in bound_and_containing()) {
            prop_assert!((b1 * b2).contains(v1 * v2, crate::ATol::default()));
        }

        #[test]
        fn pow((b, v) in bound_and_containing()) {
            prop_assert!(b.pow(2).contains(v.powi(2), crate::ATol::default()));
            prop_assert!(b.pow(3).contains(v.powi(3), crate::ATol::default()));
        }
    }
}
