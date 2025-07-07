use anyhow::bail;
use ordered_float::NotNan;
use std::ops::{Add, Deref, Neg, Sub};

use crate::Coefficient;

/// Absolute tolerance
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ATol(NotNan<f64>);

impl Deref for ATol {
    type Target = f64;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ATol {
    pub fn new(value: f64) -> anyhow::Result<Self> {
        if value <= 0.0 {
            bail!("ATol must be positive: {value}");
        }
        Ok(ATol(NotNan::new(value)?))
    }

    pub fn into_inner(&self) -> f64 {
        self.0.into_inner()
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
        ATol(NotNan::new(1e-6).unwrap())
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
