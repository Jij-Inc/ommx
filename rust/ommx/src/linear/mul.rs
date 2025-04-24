use std::ops::Mul;

use super::*;

impl Mul<Coefficient> for Linear {
    type Output = Self;
    fn mul(self, rhs: Coefficient) -> Self::Output {
        let mut result = self.clone();
        for (_, c) in &mut result.terms {
            *c *= rhs;
        }
        result.constant *= rhs.into();
        result
    }
}
