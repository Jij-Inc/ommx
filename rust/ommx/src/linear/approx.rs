use super::*;
use ::approx::AbsDiffEq;

/// Compare two linear functions in sup-norm.
impl AbsDiffEq for Linear {
    type Epsilon = f64;
    fn default_epsilon() -> Self::Epsilon {
        f64::EPSILON
    }
    fn abs_diff_eq(&self, other: &Self, epsilon: Self::Epsilon) -> bool {
        let residual = self - other;
        residual
            .max_coefficient_abs()
            .is_none_or(|max| max <= epsilon)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::approx::assert_abs_diff_eq;
    use maplit::hashmap;

    #[test]
    fn test_abs_diff_eq() {
        let zero = Linear::default();
        let small = Linear {
            terms: hashmap! {
                1.into() => 1e-9.try_into().unwrap(),
            },
            constant: Offset::default(),
        };
        assert_abs_diff_eq!(small, zero, epsilon = 1e-8);
    }
}
