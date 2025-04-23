use ordered_float::NotNan;
use std::ops::Deref;

#[derive(Debug, thiserror::Error)]
pub enum OffsetError {
    #[error("Offset must be finite")]
    Infinite,
    #[error("Offset must be not NaN")]
    NaN,
}

/// Offset of polynomial
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(transparent)]
pub struct Offset(NotNan<f64>);

impl Offset {
    pub fn into_inner(self) -> f64 {
        self.0.into_inner()
    }
}

impl From<Offset> for f64 {
    fn from(offset: Offset) -> Self {
        offset.into_inner()
    }
}

impl Deref for Offset {
    type Target = f64;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl TryFrom<f64> for Offset {
    type Error = OffsetError;
    fn try_from(value: f64) -> Result<Self, Self::Error> {
        if value.is_nan() {
            return Err(OffsetError::NaN);
        }
        if !value.is_finite() {
            return Err(OffsetError::Infinite);
        }
        Ok(Self(NotNan::new(value).unwrap())) // Safe because we checked the value is not NaN
    }
}
