use super::*;
use ::approx::AbsDiffEq;

impl AbsDiffEq for Function {
    type Epsilon = f64;

    fn default_epsilon() -> Self::Epsilon {
        1e-9
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
        let f = Function::from(Coefficient::try_from(1.0).unwrap());
        let g = Function::from(Coefficient::try_from(1.0 + 1e-10).unwrap());
        assert_abs_diff_eq!(f, g);
    }
}
