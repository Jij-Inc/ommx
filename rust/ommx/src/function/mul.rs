use super::*;
use crate::CoefficientError;
use std::ops::Mul;

impl Function {
    pub fn one() -> Self {
        Function::Constant(Coefficient::one())
    }

    /// Scale this function in place.
    ///
    /// This fast path is not atomic: if a coefficient operation fails, `self`
    /// may already have been partially modified.
    pub(crate) fn try_scale_assign_in_place(
        &mut self,
        rhs: Coefficient,
    ) -> Result<(), CoefficientError> {
        match self {
            Function::Zero => {}
            Function::Constant(c) => {
                if let Some(coefficient) = (*c * rhs)? {
                    *c = coefficient;
                } else {
                    *self = Function::Zero;
                }
            }
            Function::Linear(l) => l.try_scale_assign_in_place(rhs)?,
            Function::Quadratic(q) => q.try_scale_assign_in_place(rhs)?,
            Function::Polynomial(p) => p.try_scale_assign_in_place(rhs)?,
        }
        Ok(())
    }

    /// Multiply this function in place.
    ///
    /// This fast path is not atomic: if a coefficient operation fails, `self`
    /// may already have been partially modified.
    pub(crate) fn try_mul_assign_in_place(
        &mut self,
        rhs: &Function,
    ) -> Result<(), CoefficientError> {
        match rhs {
            Function::Zero => *self = Function::Zero,
            Function::Constant(c) => self.try_scale_assign_in_place(*c)?,
            Function::Linear(l) => self.try_mul_linear_assign_in_place(l)?,
            Function::Quadratic(q) => self.try_mul_quadratic_assign_in_place(q)?,
            Function::Polynomial(p) => self.try_mul_polynomial_ref_assign_in_place(p)?,
        }
        Ok(())
    }

    pub(crate) fn try_mul_polynomial_assign_in_place(
        &mut self,
        rhs: Polynomial,
    ) -> Result<(), CoefficientError> {
        let lhs = std::mem::take(self);
        *self = match lhs {
            Function::Zero => Function::Zero,
            Function::Constant(c) => Function::Polynomial((rhs * c)?),
            Function::Linear(l) => Function::Polynomial((&l * &rhs)?),
            Function::Quadratic(q) => Function::Polynomial((&q * &rhs)?),
            Function::Polynomial(p) => Function::Polynomial((&p * &rhs)?),
        };
        Ok(())
    }

    fn try_mul_linear_assign_in_place(&mut self, rhs: &Linear) -> Result<(), CoefficientError> {
        let lhs = std::mem::take(self);
        *self = match lhs {
            Function::Zero => Function::Zero,
            Function::Constant(c) => Function::Linear((rhs.clone() * c)?),
            Function::Linear(l) => Function::Quadratic((&l * rhs)?),
            Function::Quadratic(q) => Function::Polynomial((&q * rhs)?),
            Function::Polynomial(p) => Function::Polynomial((&p * rhs)?),
        };
        Ok(())
    }

    fn try_mul_quadratic_assign_in_place(
        &mut self,
        rhs: &Quadratic,
    ) -> Result<(), CoefficientError> {
        let lhs = std::mem::take(self);
        *self = match lhs {
            Function::Zero => Function::Zero,
            Function::Constant(c) => Function::Quadratic((rhs.clone() * c)?),
            Function::Linear(l) => Function::Polynomial((&l * rhs)?),
            Function::Quadratic(q) => Function::Polynomial((&q * rhs)?),
            Function::Polynomial(p) => Function::Polynomial((&p * rhs)?),
        };
        Ok(())
    }

    fn try_mul_polynomial_ref_assign_in_place(
        &mut self,
        rhs: &Polynomial,
    ) -> Result<(), CoefficientError> {
        let lhs = std::mem::take(self);
        *self = match lhs {
            Function::Zero => Function::Zero,
            Function::Constant(c) => Function::Polynomial((rhs * c)?),
            Function::Linear(l) => Function::Polynomial((&l * rhs)?),
            Function::Quadratic(q) => Function::Polynomial((&q * rhs)?),
            Function::Polynomial(p) => Function::Polynomial((&p * rhs)?),
        };
        Ok(())
    }
}

impl Mul<Coefficient> for Function {
    type Output = Result<Self, CoefficientError>;

    fn mul(self, rhs: Coefficient) -> Self::Output {
        let mut out = self;
        out.try_scale_assign_in_place(rhs)?;
        Ok(out.normalize())
    }
}

impl Mul<Coefficient> for &Function {
    type Output = Result<Function, CoefficientError>;

    fn mul(self, rhs: Coefficient) -> Self::Output {
        self.clone() * rhs
    }
}

impl Mul<&Coefficient> for Function {
    type Output = Result<Self, CoefficientError>;

    fn mul(self, rhs: &Coefficient) -> Self::Output {
        self * *rhs
    }
}

impl Mul<&Coefficient> for &Function {
    type Output = Result<Function, CoefficientError>;

    fn mul(self, rhs: &Coefficient) -> Self::Output {
        self.clone() * *rhs
    }
}

impl Mul<Function> for Coefficient {
    type Output = Result<Function, CoefficientError>;

    fn mul(self, rhs: Function) -> Self::Output {
        rhs * self
    }
}

impl Mul<&Function> for Coefficient {
    type Output = Result<Function, CoefficientError>;

    fn mul(self, rhs: &Function) -> Self::Output {
        rhs.clone() * self
    }
}

impl Mul for Function {
    type Output = Result<Self, CoefficientError>;

    fn mul(self, rhs: Function) -> Self::Output {
        let mut out = self;
        out.try_mul_assign_in_place(&rhs)?;
        Ok(out.normalize())
    }
}

impl Mul for &Function {
    type Output = Result<Function, CoefficientError>;

    fn mul(self, rhs: Self) -> Self::Output {
        self.clone() * rhs.clone()
    }
}

impl Mul<Function> for &Function {
    type Output = Result<Function, CoefficientError>;

    fn mul(self, rhs: Function) -> Self::Output {
        self.clone() * rhs
    }
}

impl Mul<&Function> for Function {
    type Output = Result<Function, CoefficientError>;

    fn mul(self, rhs: &Function) -> Self::Output {
        self * rhs.clone()
    }
}

macro_rules! impl_mul_polynomial_rhs {
    ($rhs:ty) => {
        impl Mul<$rhs> for Function {
            type Output = Result<Function, CoefficientError>;

            fn mul(self, rhs: $rhs) -> Self::Output {
                let mut out = self;
                out.try_mul_assign_in_place(&Function::from(rhs.clone()))?;
                Ok(out.normalize())
            }
        }

        impl Mul<$rhs> for &Function {
            type Output = Result<Function, CoefficientError>;

            fn mul(self, rhs: $rhs) -> Self::Output {
                self.clone() * rhs
            }
        }
    };
}

impl_mul_polynomial_rhs!(Linear);
impl_mul_polynomial_rhs!(&Linear);
impl_mul_polynomial_rhs!(Quadratic);
impl_mul_polynomial_rhs!(&Quadratic);
impl_mul_polynomial_rhs!(Polynomial);
impl_mul_polynomial_rhs!(&Polynomial);

#[cfg(test)]
mod tests {
    use super::*;
    use ::approx::assert_abs_diff_eq;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn mul_ref(a in any::<Function>(), b in any::<Function>()) {
            let ans = (a.clone() * b.clone()).unwrap();
            assert_abs_diff_eq!((&a * &b).unwrap(), ans);
            assert_abs_diff_eq!((a.clone() * &b).unwrap(), ans);
            assert_abs_diff_eq!((&a * b).unwrap(), ans);
        }

        #[test]
        fn zero(a in any::<Function>()) {
            assert_abs_diff_eq!((&a * Function::zero()).unwrap(), Function::zero());
            assert_abs_diff_eq!((Function::zero() * &a).unwrap(), Function::zero());
        }

        #[test]
        fn mul_commutative(a in any::<Function>(), b in any::<Function>()) {
            assert_abs_diff_eq!((&a * &b).unwrap(), (&b * &a).unwrap());
        }

        #[test]
        fn mul_associative(a in any::<Function>(), b in any::<Function>(), c in any::<Function>()) {
            assert_abs_diff_eq!((&a * (&b * &c).unwrap()).unwrap(), ((&a * &b).unwrap() * &c).unwrap());
        }
    }
}
