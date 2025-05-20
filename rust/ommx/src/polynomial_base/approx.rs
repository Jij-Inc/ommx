use super::*;
use ::approx::AbsDiffEq;

/// Compare two polynomial by maximum coefficient difference.
impl<M: Monomial> AbsDiffEq for PolynomialBase<M> {
    type Epsilon = crate::ATol;
    fn default_epsilon() -> Self::Epsilon {
        crate::ATol::default()
    }
    fn abs_diff_eq(&self, other: &Self, epsilon: Self::Epsilon) -> bool {
        let residual = self - other;
        residual
            .max_coefficient_abs()
            .is_none_or(|max| max <= *epsilon)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::approx::assert_abs_diff_eq;

    #[test]
    fn test_abs_diff_eq() {
        let zero = PolynomialBase::default();
        let small = PolynomialBase {
            terms: [(
                LinearMonomial::Variable(1.into()),
                1e-11.try_into().unwrap(),
            )]
            .into_iter()
            .collect(),
        };
        assert_abs_diff_eq!(small, zero);
    }
}
