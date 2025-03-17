use crate::{
    macros::{impl_add_from, impl_add_inverse, impl_mul_inverse},
    v1,
};
use num::Zero;
use std::ops::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bound {
    pub lower: f64,
    pub upper: f64,
}

impl Default for Bound {
    fn default() -> Self {
        Self {
            lower: f64::NEG_INFINITY,
            upper: f64::INFINITY,
        }
    }
}

impl From<v1::Bound> for Bound {
    fn from(bound: v1::Bound) -> Self {
        Self {
            lower: bound.lower,
            upper: bound.upper,
        }
    }
}

impl From<f64> for Bound {
    fn from(a: f64) -> Self {
        Self { lower: a, upper: a }
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
    /// `(-inf, inf)`
    pub fn no_bound() -> Self {
        Self {
            lower: f64::NEG_INFINITY,
            upper: f64::INFINITY,
        }
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
