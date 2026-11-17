use crate::{
    macros::{impl_add_inverse, impl_mul_inverse},
    parse::{Parse, ParseError},
    v1, ATol,
};
use approx::AbsDiffEq;
use num::Zero;
use proptest::prelude::*;
use std::ops::*;
use thiserror::Error;

mod bounds;
pub use bounds::Bounds;
mod integer_value;
pub use integer_value::IntegerValueParameters;

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

/// Extract an interval bound or the hull of a validated finite domain.
///
/// Invalid wire domains produce [`ParseError`]. Labels and substituted values
/// do not affect this domain-only conversion.
impl TryFrom<&v1::DecisionVariable> for Bound {
    type Error = ParseError;
    fn try_from(v: &v1::DecisionVariable) -> Result<Self, Self::Error> {
        if v.kind() == v1::decision_variable::Kind::FiniteDomain || v.finite_domain.is_some() {
            return crate::decision_variable::parse::parse_domain(
                v.id.into(),
                v.kind(),
                v.bound.clone(),
                v.finite_domain.clone(),
            )
            .map(|variable| variable.bound());
        }
        if let Some(bound) = &v.bound {
            bound.clone().parse(&())
        } else if v.kind() == v1::decision_variable::Kind::Binary {
            Ok(Self::of_binary())
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

    /// Generate finite values within this bound for tests and debugging.
    ///
    /// For infinite intervals, `unbounded_sampling_radius` is the radius around
    /// the point in the bound closest to zero. The sampling window is the
    /// intersection of that neighborhood with the bound. For example, radius
    /// 100 gives `[-100, 100]` for an unbounded variable, `[101, 201]` for
    /// `[101, +inf)`, and `[-50, 100]` for `[-50, +inf)`.
    /// Finite intervals are not restricted by this radius. Values whose bound
    /// residuals overflow are excluded, consistently with [`Self::contains`].
    /// The window saturates at finite `f64` limits and may collapse to one
    /// representable value.
    /// Endpoints are deliberately emphasized; no uniform distribution or stable
    /// sequence across versions is promised.
    ///
    /// # Panics
    /// Panics if `unbounded_sampling_radius` is not finite and strictly positive.
    pub fn arbitrary_containing(&self, unbounded_sampling_radius: f64) -> BoxedStrategy<f64> {
        let (mut lower, mut upper) = self.finite_generation_window(unbounded_sampling_radius);
        let atol = ATol::default();
        if !self.contains(lower, atol) || !self.contains(upper, atol) {
            // Bound membership also requires finite residuals. Intersect with
            // the original window so tolerance never enlarges the exact domain.
            let (finite_lower, finite_upper) = self.finite_feasible_extrema(atol);
            lower = lower.max(finite_lower);
            upper = upper.min(finite_upper);
        }
        if lower == upper {
            Just(lower).boxed()
        } else {
            prop_oneof![Just(lower), Just(upper), lower..=upper].boxed()
        }
    }

    // Keep the window inside the domain even when its endpoint is far from zero.
    fn finite_generation_window(&self, unbounded_sampling_radius: f64) -> (f64, f64) {
        assert!(
            unbounded_sampling_radius.is_finite() && unbounded_sampling_radius > 0.0,
            "Unbounded sampling radius must be finite and positive"
        );
        match (self.lower, self.upper) {
            (f64::NEG_INFINITY, f64::INFINITY) => {
                (-unbounded_sampling_radius, unbounded_sampling_radius)
            }
            (f64::NEG_INFINITY, upper) => {
                let upper = upper.min(unbounded_sampling_radius);
                (
                    (upper.min(0.0) - unbounded_sampling_radius).max(-f64::MAX),
                    upper,
                )
            }
            (lower, f64::INFINITY) => {
                let lower = lower.max(-unbounded_sampling_radius);
                (
                    lower,
                    (lower.max(0.0) + unbounded_sampling_radius).min(f64::MAX),
                )
            }
            (lower, upper) => (lower, upper),
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

    // Independent of Bound::arbitrary(): that intentionally uses small finite
    // endpoints for algebra tests. Exercise the full f64 representation here.
    fn generation_bounds() -> impl Strategy<Value = Bound> {
        let endpoint = prop_oneof![
            Just(f64::NEG_INFINITY),
            Just(f64::INFINITY),
            Just(-f64::MAX),
            Just(f64::MAX),
            Just(101.0),
            Just(-101.0),
            any::<f64>().prop_filter("finite endpoint", |x| x.is_finite()),
        ];
        (endpoint.clone(), endpoint)
            .prop_filter_map("valid bound", |(a, b)| Bound::new(a.min(b), a.max(b)).ok())
    }

    proptest! {
        #[test]
        fn generated_values_and_shrinks_stay_in_full_range_domains(
            bound in generation_bounds(),
            unbounded_sampling_radius in prop_oneof![Just(1.0), Just(100.0), Just(f64::MAX)],
        ) {
            use proptest::strategy::ValueTree;
            let mut runner = crate::random::Rng::deterministic();
            for _ in 0..8 {
                let mut tree = bound.arbitrary_containing(unbounded_sampling_radius).new_tree(&mut runner).unwrap();
                loop {
                    let value = tree.current();
                    prop_assert!(value.is_finite());
                    prop_assert!(bound.lower() <= value && value <= bound.upper());
                    prop_assert!(bound.contains(value, ATol::default()));
                    if !tree.simplify() { break; }
                }
            }
            let integers = IntegerValueParameters::new(bound, 100);
            prop_assert_eq!(integers.is_ok(), bound.lower().ceil() <= bound.upper().floor());
            if let Ok(parameters) = integers {
                let strategy = parameters.strategy();
                for _ in 0..8 {
                    let mut tree = strategy.new_tree(&mut runner).unwrap();
                    loop {
                        let value = tree.current();
                        prop_assert!(value.is_finite());
                        prop_assert_eq!(value, value.round());
                        prop_assert!(bound.lower() <= value && value <= bound.upper());
                        prop_assert!(bound.contains(value, ATol::default()));
                        if !tree.simplify() { break; }
                    }
                }
            }
        }
    }

    fn finite_domain_wire(values: Vec<f64>) -> v1::DecisionVariable {
        v1::DecisionVariable {
            kind: v1::decision_variable::Kind::FiniteDomain.into(),
            finite_domain: Some(v1::FiniteDomain { values }),
            ..Default::default()
        }
    }

    #[test]
    fn finite_domain_wire_bound_is_the_validated_hull() {
        for (values, lower, upper) in [
            (vec![0.1, 0.5, 1.0], 0.1, 1.0),
            (vec![1.0, 0.1, 0.5], 0.1, 1.0),
            (vec![0.5], 0.5, 0.5),
            (vec![-f64::MAX, f64::MAX], -f64::MAX, f64::MAX),
        ] {
            let wire = finite_domain_wire(values);
            assert_eq!(
                Bound::try_from(&wire).unwrap(),
                Bound::new(lower, upper).unwrap()
            );
        }
    }

    #[test]
    fn finite_domain_wire_bound_rejects_invalid_values() {
        for values in [
            vec![],
            vec![0.1, 0.1],
            vec![-0.0, 0.0],
            vec![f64::NAN],
            vec![f64::NEG_INFINITY],
            vec![f64::INFINITY],
        ] {
            assert!(Bound::try_from(&finite_domain_wire(values)).is_err());
        }
    }

    #[test]
    fn finite_domain_wire_bound_rejects_inconsistent_representations() {
        let valid = finite_domain_wire(vec![0.1, 0.5, 1.0]);
        let mut missing = valid.clone();
        missing.finite_domain = None;
        let mut both = valid.clone();
        both.bound = Some(Bound::new(0.1, 1.0).unwrap().into());
        let mut wrong_kind = valid;
        wrong_kind.kind = v1::decision_variable::Kind::Continuous.into();
        for wire in [missing, both, wrong_kind] {
            assert!(Bound::try_from(&wire).is_err());
        }
    }

    #[test]
    fn finite_domain_wire_bound_does_not_validate_evaluated_values() {
        let mut wire = finite_domain_wire(vec![0.1, 0.5, 1.0]);
        wire.substituted_value = Some(0.75);
        assert_eq!(
            Bound::try_from(&wire).unwrap(),
            Bound::new(0.1, 1.0).unwrap()
        );
    }

    #[test]
    fn wire_bound_parse_error_preserves_validation_source() {
        let error = Bound::try_from(&finite_domain_wire(vec![])).unwrap_err();
        assert!(matches!(
            error.source().and_then(|e| e.downcast_ref::<crate::DecisionVariableError>()),
            Some(crate::DecisionVariableError::InvalidDefinition { source, .. })
                if matches!(source.as_ref(), crate::DecisionVariableError::EmptyFiniteDomain)
        ));

        let wire = v1::DecisionVariable {
            kind: v1::decision_variable::Kind::Continuous.into(),
            bound: Some(v1::Bound {
                lower: 2.0,
                upper: 1.0,
            }),
            ..Default::default()
        };
        let error = Bound::try_from(&wire).unwrap_err();
        assert!(matches!(
            error.source().and_then(|e| e.downcast_ref::<BoundError>()),
            Some(BoundError::UpperSmallerThanLower { .. })
        ));
    }

    #[test]
    fn wire_bound_preserves_interval_and_binary_defaults() {
        for kind in [
            v1::decision_variable::Kind::Continuous,
            v1::decision_variable::Kind::Integer,
            v1::decision_variable::Kind::SemiContinuous,
            v1::decision_variable::Kind::SemiInteger,
            v1::decision_variable::Kind::Binary,
        ] {
            let mut wire = v1::DecisionVariable {
                kind: kind.into(),
                ..Default::default()
            };
            let expected = if kind == v1::decision_variable::Kind::Binary {
                Bound::of_binary()
            } else {
                Bound::unbounded()
            };
            assert_eq!(Bound::try_from(&wire).unwrap(), expected);

            let explicit = Bound::new(0.25, 0.75).unwrap();
            wire.bound = Some(explicit.into());
            assert_eq!(Bound::try_from(&wire).unwrap(), explicit);
        }
    }

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
