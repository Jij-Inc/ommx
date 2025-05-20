use anyhow::bail;
use ordered_float::NotNan;
use std::ops::Deref;

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
        self.0 == NotNan::new(*other).unwrap()
    }
}

impl PartialOrd<f64> for ATol {
    fn partial_cmp(&self, other: &f64) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(&NotNan::new(*other).ok()?)
    }
}
