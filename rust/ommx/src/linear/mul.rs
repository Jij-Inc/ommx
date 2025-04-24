use std::ops::Mul;

use super::*;

impl Mul<Coefficient> for Linear {
    type Output = Self;
    fn mul(self, rhs: Coefficient) -> Self::Output {
        let mut result = self.clone();
        for c in result.terms.values_mut() {
            *c *= rhs;
        }
        result.constant *= rhs.into();
        result
    }
}
