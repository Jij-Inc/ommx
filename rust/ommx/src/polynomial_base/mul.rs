use super::*;
use crate::{Coefficient, LinearMonomial, MonomialDyn, QuadraticMonomial};
use std::ops::{Mul, MulAssign};

impl<M: Monomial> MulAssign<Coefficient> for PolynomialBase<M> {
    fn mul_assign(&mut self, rhs: Coefficient) {
        for coefficient in self.terms.values_mut() {
            *coefficient *= rhs;
        }
    }
}

impl<M: Monomial> Mul<Coefficient> for PolynomialBase<M> {
    type Output = Self;
    fn mul(mut self, rhs: Coefficient) -> Self::Output {
        self *= rhs;
        self
    }
}

impl<M: Monomial> Mul<PolynomialBase<M>> for Coefficient {
    type Output = PolynomialBase<M>;
    fn mul(self, mut rhs: PolynomialBase<M>) -> Self::Output {
        rhs *= self;
        rhs
    }
}

// Add support for Coefficient * Monomial operations for specific monomial types
impl Mul<LinearMonomial> for Coefficient {
    type Output = PolynomialBase<LinearMonomial>;
    fn mul(self, rhs: LinearMonomial) -> Self::Output {
        self * PolynomialBase::from(rhs)
    }
}

impl Mul<Coefficient> for LinearMonomial {
    type Output = PolynomialBase<LinearMonomial>;
    fn mul(self, rhs: Coefficient) -> Self::Output {
        rhs * self
    }
}

impl Mul<QuadraticMonomial> for Coefficient {
    type Output = PolynomialBase<QuadraticMonomial>;
    fn mul(self, rhs: QuadraticMonomial) -> Self::Output {
        self * PolynomialBase::from(rhs)
    }
}

impl Mul<Coefficient> for QuadraticMonomial {
    type Output = PolynomialBase<QuadraticMonomial>;
    fn mul(self, rhs: Coefficient) -> Self::Output {
        rhs * self
    }
}

impl Mul<MonomialDyn> for Coefficient {
    type Output = PolynomialBase<MonomialDyn>;
    fn mul(self, rhs: MonomialDyn) -> Self::Output {
        self * PolynomialBase::from(rhs)
    }
}

impl Mul<Coefficient> for MonomialDyn {
    type Output = PolynomialBase<MonomialDyn>;
    fn mul(self, rhs: Coefficient) -> Self::Output {
        rhs * self
    }
}

impl Mul for LinearMonomial {
    type Output = QuadraticMonomial;
    fn mul(self, rhs: Self) -> Self::Output {
        use LinearMonomial::*;
        match (self, rhs) {
            (Constant, Constant) => QuadraticMonomial::Constant,
            (Constant, Variable(id)) => QuadraticMonomial::Linear(id),
            (Variable(id), Constant) => QuadraticMonomial::Linear(id),
            (Variable(id1), Variable(id2)) => QuadraticMonomial::new_pair(id1, id2),
        }
    }
}

impl Mul<LinearMonomial> for QuadraticMonomial {
    type Output = MonomialDyn;
    fn mul(self, rhs: LinearMonomial) -> Self::Output {
        self.iter().chain(rhs.iter()).collect()
    }
}

impl Mul<QuadraticMonomial> for LinearMonomial {
    type Output = MonomialDyn;
    fn mul(self, rhs: QuadraticMonomial) -> Self::Output {
        rhs.mul(self)
    }
}

impl Mul for QuadraticMonomial {
    type Output = MonomialDyn;
    fn mul(self, rhs: QuadraticMonomial) -> Self::Output {
        self.iter().chain(rhs.iter()).collect()
    }
}

impl Mul<MonomialDyn> for LinearMonomial {
    type Output = MonomialDyn;
    fn mul(self, other: MonomialDyn) -> Self::Output {
        other * self
    }
}

impl Mul<MonomialDyn> for QuadraticMonomial {
    type Output = MonomialDyn;
    fn mul(self, other: MonomialDyn) -> Self::Output {
        other * self
    }
}

impl<M1, M2, N> Mul<&PolynomialBase<M2>> for &PolynomialBase<M1>
where
    M1: Monomial + Mul<M2, Output = N>,
    M2: Monomial,
    N: Monomial,
{
    type Output = PolynomialBase<N>;
    fn mul(self, rhs: &PolynomialBase<M2>) -> Self::Output {
        let mut out = Self::Output::default();
        for (lhs_m, lhs_c) in self {
            for (rhs_m, rhs_c) in rhs {
                out.add_term(lhs_m.clone() * rhs_m.clone(), *lhs_c * *rhs_c);
            }
        }
        out
    }
}

impl<M1, M2> MulAssign<&PolynomialBase<M2>> for PolynomialBase<M1>
where
    M1: Monomial + Mul<M2, Output = M1>,
    M2: Monomial,
{
    fn mul_assign(&mut self, rhs: &PolynomialBase<M2>) {
        *self = &*self * rhs;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VariableID;
    use ::approx::assert_abs_diff_eq;

    #[test]
    fn test_coefficient_monomial_mul() {
        // Test Coefficient * LinearMonomial
        let coef = Coefficient::try_from(2.0).unwrap();
        let x1 = LinearMonomial::Variable(VariableID::from(1));
        let result = coef * x1;

        // Expected result is the same as coefficient * monomial
        let expected = coef * x1;
        assert_abs_diff_eq!(result, expected);

        // Test LinearMonomial * Coefficient
        let result = x1 * coef;
        // Should be the same as coefficient * monomial
        let expected = coef * x1;
        assert_abs_diff_eq!(result, expected);

        // Test Coefficient * QuadraticMonomial
        let q1 = QuadraticMonomial::Linear(VariableID::from(1));
        let result = coef * q1;

        // Expected result is the same as coefficient * monomial
        let expected = coef * q1;
        assert_abs_diff_eq!(result, expected);

        // Test QuadraticMonomial * Coefficient
        let result = q1 * coef;
        // Should be the same as coefficient * monomial
        let expected = coef * q1;
        assert_abs_diff_eq!(result, expected);
    }
}
