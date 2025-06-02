use super::*;
use crate::ATol;
use ::approx::AbsDiffEq;

impl AbsDiffEq for Function {
    type Epsilon = ATol;

    fn default_epsilon() -> Self::Epsilon {
        ATol::default()
    }

    fn abs_diff_eq(&self, other: &Self, epsilon: Self::Epsilon) -> bool {
        // Compute residual function and check max absolute coefficient
        let diff = self.clone() - other.clone();
        diff.values()
            .map(|c| c.abs())
            .max()
            .is_none_or(|c| c <= epsilon)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::approx::assert_abs_diff_eq;

    #[test]
    fn test_abs_diff_eq() {
        let f = Function::from(crate::coeff!(1.0));
        let g = Function::from(crate::coeff!(1.0000000001));
        assert_abs_diff_eq!(f, g);
    }
}
