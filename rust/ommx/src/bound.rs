use crate::{
    macros::{impl_add_inverse, impl_mul_inverse},
    parse::{Parse, ParseError, RawParseError},
    v1::{self, State},
    VariableID,
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
    pub fn arbitrary_containing(&self) -> BoxedStrategy<f64> {
        // RangeInclusive::arbitrary() does not support infinite range
        match (self.lower, self.upper) {
            (f64::NEG_INFINITY, f64::INFINITY) => f64::arbitrary().boxed(),
            (f64::NEG_INFINITY, upper) => prop_oneof![
                f64::arbitrary().prop_filter("upper", move |&x| x <= upper),
                Just(upper)
            ]
            .boxed(),
            (lower, f64::INFINITY) => prop_oneof![
                f64::arbitrary().prop_filter("lower", move |&x| x >= lower),
                Just(lower)
            ]
            .boxed(),
            (lower, upper) => {
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

pub fn arbitrary_bounds(ids: impl Iterator<Item = VariableID>) -> BoxedStrategy<Bounds> {
    let mut strategy = Just(HashMap::new()).boxed();
    for id in ids {
        strategy = (strategy, Bound::arbitrary())
            .prop_map(move |(mut bounds, bound)| {
                bounds.insert(id, bound);
                bounds
            })
            .boxed();
    }
    strategy
}

pub fn arbitrary_state_within_bounds(bounds: &Bounds) -> BoxedStrategy<State> {
    let mut stratety = Just(HashMap::new()).boxed();
    for (id, bound) in bounds {
        let raw_id = *id.deref();
        stratety = (stratety, bound.as_range())
            .prop_map(move |(mut state, value)| {
                state.insert(raw_id, value);
                state
            })
            .boxed();
    }
    stratety.prop_map(|state| state.into()).boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bound_and_containing() -> BoxedStrategy<(Bound, f64)> {
        Bound::arbitrary()
            .prop_flat_map(|bound| (Just(bound), bound.arbitrary_containing()))
            .boxed()
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
    }
}
