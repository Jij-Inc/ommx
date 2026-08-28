use super::*;
use crate::{CoefficientError, LinearMonomial, MonomialDyn, QuadraticMonomial};
use std::ops::Mul;

impl<M: Monomial> PolynomialBase<M> {
    /// Apply `f` to every coefficient in place, removing terms for which `f`
    /// returns `None` (i.e. the coefficient became zero).
    fn try_map_coefficients_in_place(
        &mut self,
        f: impl Fn(Coefficient) -> Result<Option<Coefficient>, CoefficientError>,
    ) -> Result<(), CoefficientError> {
        let mut removed = Vec::new();
        for (monomial, coefficient) in self.terms.iter_mut() {
            if let Some(mapped) = f(*coefficient)? {
                *coefficient = mapped;
            } else {
                removed.push(monomial.clone());
            }
        }
        for monomial in removed {
            self.terms.remove(&monomial);
        }
        Ok(())
    }

    pub(crate) fn try_scale_assign_in_place(
        &mut self,
        rhs: Coefficient,
    ) -> Result<(), CoefficientError> {
        self.try_map_coefficients_in_place(|coefficient| coefficient * rhs)
    }

    pub(crate) fn try_div_assign_in_place(
        &mut self,
        rhs: Coefficient,
    ) -> Result<(), CoefficientError> {
        self.try_map_coefficients_in_place(|coefficient| coefficient / rhs)
    }
}

impl<M: Monomial> Mul<Coefficient> for PolynomialBase<M> {
    type Output = Result<Self, CoefficientError>;

    fn mul(mut self, rhs: Coefficient) -> Self::Output {
        self.try_scale_assign_in_place(rhs)?;
        Ok(self)
    }
}

impl<M: Monomial> Mul<PolynomialBase<M>> for Coefficient {
    type Output = Result<PolynomialBase<M>, CoefficientError>;

    fn mul(self, rhs: PolynomialBase<M>) -> Self::Output {
        rhs * self
    }
}

impl<M: Monomial> Mul<Coefficient> for &PolynomialBase<M> {
    type Output = Result<PolynomialBase<M>, CoefficientError>;

    fn mul(self, rhs: Coefficient) -> Self::Output {
        self.clone() * rhs
    }
}

impl<M: Monomial> Mul<&PolynomialBase<M>> for Coefficient {
    type Output = Result<PolynomialBase<M>, CoefficientError>;

    fn mul(self, rhs: &PolynomialBase<M>) -> Self::Output {
        rhs.clone() * self
    }
}

macro_rules! impl_coefficient_monomial_mul {
    ($monomial:ty) => {
        impl Mul<$monomial> for Coefficient {
            type Output = Result<PolynomialBase<$monomial>, CoefficientError>;

            fn mul(self, rhs: $monomial) -> Self::Output {
                self * PolynomialBase::from(rhs)
            }
        }

        impl Mul<Coefficient> for $monomial {
            type Output = Result<PolynomialBase<$monomial>, CoefficientError>;

            fn mul(self, rhs: Coefficient) -> Self::Output {
                rhs * self
            }
        }

        impl Mul<&$monomial> for Coefficient {
            type Output = Result<PolynomialBase<$monomial>, CoefficientError>;

            fn mul(self, rhs: &$monomial) -> Self::Output {
                self * rhs.clone()
            }
        }

        impl Mul<Coefficient> for &$monomial {
            type Output = Result<PolynomialBase<$monomial>, CoefficientError>;

            fn mul(self, rhs: Coefficient) -> Self::Output {
                rhs * self.clone()
            }
        }
    };
}

impl_coefficient_monomial_mul!(LinearMonomial);
impl_coefficient_monomial_mul!(QuadraticMonomial);
impl_coefficient_monomial_mul!(MonomialDyn);

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
    type Output = Result<PolynomialBase<N>, CoefficientError>;

    fn mul(self, rhs: &PolynomialBase<M2>) -> Self::Output {
        // The product has at most n·m terms; reserving up front avoids the
        // repeated rehash-and-grow cycles of an incrementally filled map.
        let mut out =
            PolynomialBase::<N>::with_capacity(self.num_terms().saturating_mul(rhs.num_terms()));
        for (lhs_m, lhs_c) in self {
            for (rhs_m, rhs_c) in rhs {
                if let Some(coefficient) = (*lhs_c * *rhs_c)? {
                    out.add_term(lhs_m.clone() * rhs_m.clone(), coefficient)?;
                }
            }
        }
        Ok(out)
    }
}

impl<M1, M2, N> Mul<PolynomialBase<M2>> for PolynomialBase<M1>
where
    M1: Monomial + Mul<M2, Output = N>,
    M2: Monomial,
    N: Monomial,
{
    type Output = Result<PolynomialBase<N>, CoefficientError>;

    fn mul(self, rhs: PolynomialBase<M2>) -> Self::Output {
        &self * &rhs
    }
}

impl<M1, M2, N> Mul<&PolynomialBase<M2>> for PolynomialBase<M1>
where
    M1: Monomial + Mul<M2, Output = N>,
    M2: Monomial,
    N: Monomial,
{
    type Output = Result<PolynomialBase<N>, CoefficientError>;

    fn mul(self, rhs: &PolynomialBase<M2>) -> Self::Output {
        &self * rhs
    }
}

impl<M1, M2, N> Mul<PolynomialBase<M2>> for &PolynomialBase<M1>
where
    M1: Monomial + Mul<M2, Output = N>,
    M2: Monomial,
    N: Monomial,
{
    type Output = Result<PolynomialBase<N>, CoefficientError>;

    fn mul(self, rhs: PolynomialBase<M2>) -> Self::Output {
        self * &rhs
    }
}
