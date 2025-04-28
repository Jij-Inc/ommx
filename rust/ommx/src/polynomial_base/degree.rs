use derive_more::{Deref, From};
use std::{fmt, ops::Sub};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, From, Deref)]
pub struct Degree(u32);

impl Degree {
    pub fn into_inner(&self) -> u32 {
        self.0
    }
}

impl Sub<u32> for Degree {
    type Output = Self;
    fn sub(self, rhs: u32) -> Self::Output {
        Degree(self.0 - rhs)
    }
}

impl PartialEq<u32> for Degree {
    fn eq(&self, other: &u32) -> bool {
        self.0.eq(other)
    }
}

impl fmt::Display for Degree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
