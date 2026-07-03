use super::*;
use crate::{CoefficientError, LinearMonomial, MonomialDyn, QuadraticMonomial};
use std::ops::{Add, Neg, Sub};

impl<M: Monomial> PolynomialBase<M> {
    pub fn zero() -> Self {
        Self::default()
    }

    pub fn is_zero(&self) -> bool {
        self.terms.is_empty()
    }

    /// Add `rhs` to this polynomial in place.
    ///
    /// This is a fallible replacement for `AddAssign`: it returns
    /// [`CoefficientError`] when coefficient arithmetic overflows or produces
    /// NaN. The operation is intentionally not atomic. If an error is returned,
    /// terms processed before the failing coefficient may already have been
    /// updated.
    pub fn try_add_assign_in_place<N>(
        &mut self,
        rhs: &PolynomialBase<N>,
    ) -> Result<(), CoefficientError>
    where
        N: Monomial,
        M: From<N>,
    {
        for (monomial, coefficient) in &rhs.terms {
            self.add_term(M::from(monomial.clone()), *coefficient)?;
        }
        Ok(())
    }
}

impl<M1, M2> Add<&PolynomialBase<M1>> for PolynomialBase<M2>
where
    M1: Monomial,
    M2: Monomial + From<M1>,
{
    type Output = Result<Self, CoefficientError>;

    fn add(mut self, rhs: &PolynomialBase<M1>) -> Self::Output {
        self.try_add_assign_in_place(rhs)?;
        Ok(self)
    }
}

impl<M: Monomial> Add for PolynomialBase<M> {
    type Output = Result<Self, CoefficientError>;

    fn add(self, rhs: Self) -> Self::Output {
        if self.terms.len() < rhs.terms.len() {
            rhs + &self
        } else {
            self + &rhs
        }
    }
}

impl<M: Monomial> Add<Result<PolynomialBase<M>, CoefficientError>> for PolynomialBase<M> {
    type Output = Result<Self, CoefficientError>;

    fn add(self, rhs: Result<PolynomialBase<M>, CoefficientError>) -> Self::Output {
        self + rhs?
    }
}

impl<M: Monomial> Add for &PolynomialBase<M> {
    type Output = Result<PolynomialBase<M>, CoefficientError>;

    fn add(self, rhs: Self) -> Self::Output {
        if self.terms.len() < rhs.terms.len() {
            rhs.clone() + self
        } else {
            self.clone() + rhs
        }
    }
}

impl<M: Monomial> Add<Result<PolynomialBase<M>, CoefficientError>> for &PolynomialBase<M> {
    type Output = Result<PolynomialBase<M>, CoefficientError>;

    fn add(self, rhs: Result<PolynomialBase<M>, CoefficientError>) -> Self::Output {
        self.clone() + rhs?
    }
}

impl<M: Monomial> Add<PolynomialBase<M>> for &PolynomialBase<M> {
    type Output = Result<PolynomialBase<M>, CoefficientError>;

    fn add(self, rhs: PolynomialBase<M>) -> Self::Output {
        rhs + self
    }
}

impl<M: Monomial> Add<Coefficient> for PolynomialBase<M> {
    type Output = Result<Self, CoefficientError>;

    fn add(mut self, rhs: Coefficient) -> Self::Output {
        self.add_term(M::default(), rhs)?;
        Ok(self)
    }
}

impl<M: Monomial> Add<PolynomialBase<M>> for Coefficient {
    type Output = Result<PolynomialBase<M>, CoefficientError>;

    fn add(self, rhs: PolynomialBase<M>) -> Self::Output {
        rhs + self
    }
}

impl<M: Monomial> Add<Coefficient> for &PolynomialBase<M> {
    type Output = Result<PolynomialBase<M>, CoefficientError>;

    fn add(self, rhs: Coefficient) -> Self::Output {
        self.clone() + rhs
    }
}

impl<M: Monomial> Add<&PolynomialBase<M>> for Coefficient {
    type Output = Result<PolynomialBase<M>, CoefficientError>;

    fn add(self, rhs: &PolynomialBase<M>) -> Self::Output {
        self + rhs.clone()
    }
}

impl<M: Monomial> Add<M> for PolynomialBase<M> {
    type Output = Result<Self, CoefficientError>;

    fn add(mut self, rhs: M) -> Self::Output {
        self.add_term(rhs, Coefficient::one())?;
        Ok(self)
    }
}

impl<M: Monomial> Add<&M> for PolynomialBase<M> {
    type Output = Result<Self, CoefficientError>;

    fn add(mut self, rhs: &M) -> Self::Output {
        self.add_term(rhs.clone(), Coefficient::one())?;
        Ok(self)
    }
}

impl<M: Monomial> Add<M> for &PolynomialBase<M> {
    type Output = Result<PolynomialBase<M>, CoefficientError>;

    fn add(self, rhs: M) -> Self::Output {
        self.clone() + rhs
    }
}

impl<M: Monomial> Add<&M> for &PolynomialBase<M> {
    type Output = Result<PolynomialBase<M>, CoefficientError>;

    fn add(self, rhs: &M) -> Self::Output {
        self.clone() + rhs
    }
}

impl Add<&PolynomialBase<LinearMonomial>> for &PolynomialBase<MonomialDyn> {
    type Output = Result<PolynomialBase<MonomialDyn>, CoefficientError>;

    fn add(self, rhs: &PolynomialBase<LinearMonomial>) -> Self::Output {
        let mut result = self.clone();
        result.try_add_assign_in_place(rhs)?;
        Ok(result)
    }
}

impl Add<&PolynomialBase<QuadraticMonomial>> for &PolynomialBase<MonomialDyn> {
    type Output = Result<PolynomialBase<MonomialDyn>, CoefficientError>;

    fn add(self, rhs: &PolynomialBase<QuadraticMonomial>) -> Self::Output {
        let mut result = self.clone();
        result.try_add_assign_in_place(rhs)?;
        Ok(result)
    }
}

impl Add<&PolynomialBase<LinearMonomial>> for &PolynomialBase<QuadraticMonomial> {
    type Output = Result<PolynomialBase<QuadraticMonomial>, CoefficientError>;

    fn add(self, rhs: &PolynomialBase<LinearMonomial>) -> Self::Output {
        let mut result = self.clone();
        result.try_add_assign_in_place(rhs)?;
        Ok(result)
    }
}

impl<M: Monomial> Neg for PolynomialBase<M> {
    type Output = Self;

    fn neg(mut self) -> Self::Output {
        for c in self.terms.values_mut() {
            *c = -(*c);
        }
        self
    }
}

impl<M1, M2> Sub<&PolynomialBase<M1>> for PolynomialBase<M2>
where
    M1: Monomial,
    M2: Monomial + From<M1>,
{
    type Output = Result<Self, CoefficientError>;

    fn sub(mut self, rhs: &PolynomialBase<M1>) -> Self::Output {
        for (id, c) in &rhs.terms {
            self.add_term(Into::<M2>::into(id.clone()), -(*c))?;
        }
        Ok(self)
    }
}

impl<M: Monomial> Sub for PolynomialBase<M> {
    type Output = Result<Self, CoefficientError>;

    fn sub(self, rhs: Self) -> Self::Output {
        if self.terms.len() < rhs.terms.len() {
            (-rhs) + &self
        } else {
            self - &rhs
        }
    }
}

impl<M: Monomial> Sub<Result<PolynomialBase<M>, CoefficientError>> for PolynomialBase<M> {
    type Output = Result<Self, CoefficientError>;

    fn sub(self, rhs: Result<PolynomialBase<M>, CoefficientError>) -> Self::Output {
        self - rhs?
    }
}

impl<M: Monomial> Sub for &PolynomialBase<M> {
    type Output = Result<PolynomialBase<M>, CoefficientError>;

    fn sub(self, rhs: Self) -> Self::Output {
        if self.terms.len() < rhs.terms.len() {
            (-rhs.clone()) + self
        } else {
            self.clone() - rhs
        }
    }
}

impl<M: Monomial> Sub<Result<PolynomialBase<M>, CoefficientError>> for &PolynomialBase<M> {
    type Output = Result<PolynomialBase<M>, CoefficientError>;

    fn sub(self, rhs: Result<PolynomialBase<M>, CoefficientError>) -> Self::Output {
        self.clone() - rhs?
    }
}

impl<M: Monomial> Sub<PolynomialBase<M>> for &PolynomialBase<M> {
    type Output = Result<PolynomialBase<M>, CoefficientError>;

    fn sub(self, rhs: PolynomialBase<M>) -> Self::Output {
        (-rhs) + self
    }
}

impl<M: Monomial> Sub<Coefficient> for PolynomialBase<M> {
    type Output = Result<Self, CoefficientError>;

    fn sub(mut self, rhs: Coefficient) -> Self::Output {
        self.add_term(M::default(), -rhs)?;
        Ok(self)
    }
}

impl<M: Monomial> Sub<PolynomialBase<M>> for Coefficient {
    type Output = Result<PolynomialBase<M>, CoefficientError>;

    fn sub(self, rhs: PolynomialBase<M>) -> Self::Output {
        (-rhs) + self
    }
}

impl<M: Monomial> Sub<Coefficient> for &PolynomialBase<M> {
    type Output = Result<PolynomialBase<M>, CoefficientError>;

    fn sub(self, rhs: Coefficient) -> Self::Output {
        self.clone() - rhs
    }
}

impl<M: Monomial> Sub<&PolynomialBase<M>> for Coefficient {
    type Output = Result<PolynomialBase<M>, CoefficientError>;

    fn sub(self, rhs: &PolynomialBase<M>) -> Self::Output {
        self - rhs.clone()
    }
}

impl<M: Monomial> Sub<M> for PolynomialBase<M> {
    type Output = Result<Self, CoefficientError>;

    fn sub(mut self, rhs: M) -> Self::Output {
        self.add_term(rhs, -Coefficient::one())?;
        Ok(self)
    }
}

impl<M: Monomial> Sub<&M> for PolynomialBase<M> {
    type Output = Result<Self, CoefficientError>;

    fn sub(mut self, rhs: &M) -> Self::Output {
        self.add_term(rhs.clone(), -Coefficient::one())?;
        Ok(self)
    }
}

impl<M: Monomial> Sub<M> for &PolynomialBase<M> {
    type Output = Result<PolynomialBase<M>, CoefficientError>;

    fn sub(self, rhs: M) -> Self::Output {
        self.clone() - rhs
    }
}

impl<M: Monomial> Sub<&M> for &PolynomialBase<M> {
    type Output = Result<PolynomialBase<M>, CoefficientError>;

    fn sub(self, rhs: &M) -> Self::Output {
        self.clone() - rhs
    }
}

macro_rules! impl_monomial_op {
    ($op_trait:ident, $op_method:ident, $monomial:ty) => {
        impl $op_trait for $monomial {
            type Output = Result<PolynomialBase<$monomial>, CoefficientError>;

            fn $op_method(self, rhs: Self) -> Self::Output {
                PolynomialBase::from(self).$op_method(PolynomialBase::from(rhs))
            }
        }

        impl $op_trait<&$monomial> for $monomial {
            type Output = Result<PolynomialBase<$monomial>, CoefficientError>;

            fn $op_method(self, rhs: &Self) -> Self::Output {
                PolynomialBase::from(self).$op_method(PolynomialBase::from(rhs.clone()))
            }
        }

        impl $op_trait<$monomial> for &$monomial {
            type Output = Result<PolynomialBase<$monomial>, CoefficientError>;

            fn $op_method(self, rhs: $monomial) -> Self::Output {
                PolynomialBase::from(self.clone()).$op_method(PolynomialBase::from(rhs))
            }
        }

        impl $op_trait for &$monomial {
            type Output = Result<PolynomialBase<$monomial>, CoefficientError>;

            fn $op_method(self, rhs: Self) -> Self::Output {
                PolynomialBase::from(self.clone()).$op_method(PolynomialBase::from(rhs.clone()))
            }
        }

        impl $op_trait<Coefficient> for $monomial {
            type Output = Result<PolynomialBase<$monomial>, CoefficientError>;

            fn $op_method(self, rhs: Coefficient) -> Self::Output {
                PolynomialBase::from(self).$op_method(rhs)
            }
        }

        impl $op_trait<$monomial> for Coefficient {
            type Output = Result<PolynomialBase<$monomial>, CoefficientError>;

            fn $op_method(self, rhs: $monomial) -> Self::Output {
                self.$op_method(PolynomialBase::from(rhs))
            }
        }

        impl $op_trait<Coefficient> for &$monomial {
            type Output = Result<PolynomialBase<$monomial>, CoefficientError>;

            fn $op_method(self, rhs: Coefficient) -> Self::Output {
                PolynomialBase::from(self.clone()).$op_method(rhs)
            }
        }

        impl $op_trait<&$monomial> for Coefficient {
            type Output = Result<PolynomialBase<$monomial>, CoefficientError>;

            fn $op_method(self, rhs: &$monomial) -> Self::Output {
                self.$op_method(PolynomialBase::from(rhs.clone()))
            }
        }

        impl $op_trait<PolynomialBase<$monomial>> for $monomial {
            type Output = Result<PolynomialBase<$monomial>, CoefficientError>;

            fn $op_method(self, rhs: PolynomialBase<$monomial>) -> Self::Output {
                PolynomialBase::from(self).$op_method(rhs)
            }
        }

        impl $op_trait<Result<PolynomialBase<$monomial>, CoefficientError>> for $monomial {
            type Output = Result<PolynomialBase<$monomial>, CoefficientError>;

            fn $op_method(
                self,
                rhs: Result<PolynomialBase<$monomial>, CoefficientError>,
            ) -> Self::Output {
                PolynomialBase::from(self).$op_method(rhs?)
            }
        }

        impl $op_trait<&PolynomialBase<$monomial>> for $monomial {
            type Output = Result<PolynomialBase<$monomial>, CoefficientError>;

            fn $op_method(self, rhs: &PolynomialBase<$monomial>) -> Self::Output {
                PolynomialBase::from(self).$op_method(rhs)
            }
        }

        impl $op_trait<PolynomialBase<$monomial>> for &$monomial {
            type Output = Result<PolynomialBase<$monomial>, CoefficientError>;

            fn $op_method(self, rhs: PolynomialBase<$monomial>) -> Self::Output {
                PolynomialBase::from(self.clone()).$op_method(rhs)
            }
        }

        impl $op_trait<Result<PolynomialBase<$monomial>, CoefficientError>> for &$monomial {
            type Output = Result<PolynomialBase<$monomial>, CoefficientError>;

            fn $op_method(
                self,
                rhs: Result<PolynomialBase<$monomial>, CoefficientError>,
            ) -> Self::Output {
                PolynomialBase::from(self.clone()).$op_method(rhs?)
            }
        }

        impl $op_trait<&PolynomialBase<$monomial>> for &$monomial {
            type Output = Result<PolynomialBase<$monomial>, CoefficientError>;

            fn $op_method(self, rhs: &PolynomialBase<$monomial>) -> Self::Output {
                PolynomialBase::from(self.clone()).$op_method(rhs)
            }
        }
    };
}

impl_monomial_op!(Add, add, LinearMonomial);
impl_monomial_op!(Add, add, QuadraticMonomial);
impl_monomial_op!(Add, add, MonomialDyn);

impl_monomial_op!(Sub, sub, LinearMonomial);
impl_monomial_op!(Sub, sub, QuadraticMonomial);
impl_monomial_op!(Sub, sub, MonomialDyn);

#[cfg(test)]
mod tests {
    use super::*;
    use ::approx::assert_abs_diff_eq;
    use proptest::prelude::*;

    type Linear = PolynomialBase<LinearMonomial>;

    proptest! {
        #[test]
        fn add_ref(a: Linear, b: Linear) {
            let ans = (a.clone() + b.clone()).unwrap();
            assert_abs_diff_eq!((&a + &b).unwrap(), ans);
            assert_abs_diff_eq!((&a + b.clone()).unwrap(), ans);
            assert_abs_diff_eq!((a + &b).unwrap(), ans);
        }

        #[test]
        fn sub_ref(a: Linear, b: Linear) {
            let ans = (a.clone() - b.clone()).unwrap();
            assert_abs_diff_eq!((&a - &b).unwrap(), ans);
            assert_abs_diff_eq!((&a - b.clone()).unwrap(), ans);
            assert_abs_diff_eq!((a.clone() - &b).unwrap(), ans);
        }

        #[test]
        fn zero(a: Linear) {
            assert_abs_diff_eq!((&a + Linear::zero()).unwrap(), &a);
            assert_abs_diff_eq!((&a - Linear::zero()).unwrap(), &a);
            assert_abs_diff_eq!((&a - &a).unwrap(), Linear::zero());
        }

        #[test]
        fn add_commutative(a: Linear, b: Linear) {
            assert_abs_diff_eq!((&a + &b).unwrap(), (&b + &a).unwrap());
        }

        #[test]
        fn add_associative(a: Linear, b: Linear, c: Linear) {
            assert_abs_diff_eq!((&a + (&b + &c).unwrap()).unwrap(), ((&a + &b).unwrap() + &c).unwrap());
        }
    }
}
