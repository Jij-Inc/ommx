use anyhow::bail;
use ordered_float::NotNan;
use std::ops::{Add, Deref, Neg, Sub};
use std::sync::{LazyLock, RwLock};

use crate::Coefficient;

/// Absolute tolerance
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ATol(NotNan<f64>);

static DEFAULT_ATOL: LazyLock<RwLock<f64>> = LazyLock::new(|| {
    let default_value = match std::env::var("OMMX_DEFAULT_ATOL") {
        Ok(s) => {
            match s.parse::<f64>() {
                Ok(v) if v > 0.0 => {
                    log::info!("Using OMMX_DEFAULT_ATOL environment variable: {}", v);
                    v
                }
                Ok(v) => {
                    log::warn!("Invalid OMMX_DEFAULT_ATOL value (must be positive): {}. Using default 1e-6", v);
                    1e-6
                }
                Err(_) => {
                    log::warn!(
                        "Invalid OMMX_DEFAULT_ATOL value (not a number): '{}'. Using default 1e-6",
                        s
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
    pub fn new(value: f64) -> anyhow::Result<Self> {
        if value <= 0.0 {
            bail!("ATol must be positive: {value}");
        }
        Ok(ATol(NotNan::new(value)?))
    }

    pub fn into_inner(&self) -> f64 {
        self.0.into_inner()
    }

    pub fn set_default(value: f64) -> anyhow::Result<()> {
        let atol = Self::new(value)?;
        let mut default = DEFAULT_ATOL.write().unwrap();
        *default = atol.into_inner();
        log::info!("ATol default value changed to: {}", value);
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
        let default_value = *DEFAULT_ATOL.read().unwrap();
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
