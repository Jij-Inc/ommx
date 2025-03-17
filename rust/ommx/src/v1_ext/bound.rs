use crate::{
    macros::{impl_add_from, impl_add_inverse, impl_mul_inverse},
    v1::Bound,
};
use std::ops::*;

impl Copy for Bound {}

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
}
