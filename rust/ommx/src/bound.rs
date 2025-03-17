use crate::{
    macros::{impl_add_from, impl_add_inverse, impl_mul_inverse},
    parse::{Parse, ParseError, RawParseError},
    v1, VariableID,
};
use num::Zero;
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
/// let bound = Bound::from([1.0, 2.0]);
/// // Into trait
/// let bound: Bound = [1.0, 2.0].into();
/// // Single point `[1.0, 1.0]`
/// let bound = Bound::from(1.0);
/// // Default is `(-inf, inf)`
/// assert_eq!(Bound::default(), Bound::from([f64::NEG_INFINITY, f64::INFINITY]));
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
    type Error = ParseError;
    fn try_from(value: v1::Bound) -> std::result::Result<Self, Self::Error> {
        Ok(value.parse(&())?)
    }
}

impl From<f64> for Bound {
    fn from(a: f64) -> Self {
        Self { lower: a, upper: a }
    }
}

impl From<[f64; 2]> for Bound {
    fn from([lower, upper]: [f64; 2]) -> Self {
        assert!(
            lower <= upper,
            "Bound must satisfy lower({lower}) <= upper({upper})",
        );
        Self { lower, upper }
    }
}

impl Add for Bound {
    type Output = Bound;
    fn add(self, rhs: Self) -> Self::Output {
        Bound {
            lower: self.lower + rhs.lower,
            upper: self.upper + rhs.upper,
        }
    }
}
impl_add_from!(Bound, f64);
impl_add_inverse!(f64, Bound);

impl AddAssign for Bound {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Zero for Bound {
    fn zero() -> Self {
        Self::from(0.0)
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
        Bound {
            lower: a.min(b).min(c).min(d),
            upper: a.max(b).max(c).max(d),
        }
    }
}

impl Mul<f64> for Bound {
    type Output = Bound;
    fn mul(self, rhs: f64) -> Self::Output {
        if rhs >= 0.0 {
            Bound {
                lower: self.lower * rhs,
                upper: self.upper * rhs,
            }
        } else {
            Bound {
                lower: self.upper * rhs,
                upper: self.lower * rhs,
            }
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
                Bound {
                    lower: self.lower.powi(exp as i32),
                    upper: self.upper.powi(exp as i32),
                }
            } else if self.upper <= 0.0 {
                // lower <= upper <= 0
                Bound {
                    lower: self.upper.powi(exp as i32),
                    upper: self.lower.powi(exp as i32),
                }
            } else {
                // lower <= 0 <= upper
                Bound {
                    lower: 0.0,
                    upper: self.upper.powi(exp as i32),
                }
            }
        } else {
            // pow is monotonic for odd exponents
            Bound {
                lower: self.lower.powi(exp as i32),
                upper: self.upper.powi(exp as i32),
            }
        }
    }
}
