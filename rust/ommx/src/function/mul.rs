use super::operation::{associative_operation, AssociativeOperator};
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
            Function::Expression(_) => {
                if rhs != Coefficient::one() {
                    let lhs = std::mem::take(self);
                    *self = associative_operation(
                        AssociativeOperator::Mul,
                        lhs,
                        Function::Constant(rhs),
                    );
                }
            }
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
        if !self.is_polynomial() || !rhs.is_polynomial() {
            let lhs = std::mem::take(self);
            *self = match (&lhs, rhs) {
                (_, Function::Constant(c)) if *c == Coefficient::one() => lhs,
                (Function::Constant(c), _) if *c == Coefficient::one() => rhs.clone(),
                _ => associative_operation(AssociativeOperator::Mul, lhs, rhs.clone()),
            };
            return Ok(());
        }
        match rhs {
            Function::Zero => *self = Function::Zero,
            Function::Constant(c) => self.try_scale_assign_in_place(*c)?,
            Function::Linear(l) => self.try_mul_linear_assign_in_place(l)?,
            Function::Quadratic(q) => self.try_mul_quadratic_assign_in_place(q)?,
            Function::Polynomial(p) => self.try_mul_polynomial_ref_assign_in_place(p)?,
            Function::Expression(_) => {
                unreachable!("composite functions were handled above")
            }
        }
        Ok(())
    }

    /// Multiply by an owned function without routing it through a borrowed
    /// path that would clone it again for expression construction or scaling.
    fn try_mul_assign_owned_in_place(&mut self, rhs: Function) -> Result<(), CoefficientError> {
        let lhs = std::mem::take(self);
        *self = if !lhs.is_polynomial() || !rhs.is_polynomial() {
            match (lhs, rhs) {
                (lhs, Function::Constant(c)) if c == Coefficient::one() => lhs,
                (Function::Constant(c), rhs) if c == Coefficient::one() => rhs,
                (lhs, rhs) => associative_operation(AssociativeOperator::Mul, lhs, rhs),
            }
        } else {
            match (lhs, rhs) {
                (Function::Zero, _) | (_, Function::Zero) => Function::Zero,
                (Function::Constant(lhs), Function::Constant(rhs)) => {
                    if let Some(coefficient) = (lhs * rhs)? {
                        Function::Constant(coefficient)
                    } else {
                        Function::Zero
                    }
                }
                (Function::Constant(c), Function::Linear(l))
                | (Function::Linear(l), Function::Constant(c)) => Function::Linear((l * c)?),
                (Function::Constant(c), Function::Quadratic(q))
                | (Function::Quadratic(q), Function::Constant(c)) => Function::Quadratic((q * c)?),
                (Function::Constant(c), Function::Polynomial(p))
                | (Function::Polynomial(p), Function::Constant(c)) => {
                    Function::Polynomial((p * c)?)
                }
                (Function::Linear(lhs), Function::Linear(rhs)) => {
                    Function::Quadratic((&lhs * &rhs)?)
                }
                (Function::Linear(l), Function::Quadratic(q)) => Function::Polynomial((&l * &q)?),
                (Function::Quadratic(q), Function::Linear(l)) => Function::Polynomial((&q * &l)?),
                (Function::Linear(l), Function::Polynomial(p)) => Function::Polynomial((&l * &p)?),
                (Function::Polynomial(p), Function::Linear(l)) => Function::Polynomial((&p * &l)?),
                (Function::Quadratic(lhs), Function::Quadratic(rhs)) => {
                    Function::Polynomial((&lhs * &rhs)?)
                }
                (Function::Quadratic(q), Function::Polynomial(p)) => {
                    Function::Polynomial((&q * &p)?)
                }
                (Function::Polynomial(p), Function::Quadratic(q)) => {
                    Function::Polynomial((&p * &q)?)
                }
                (Function::Polynomial(lhs), Function::Polynomial(rhs)) => {
                    Function::Polynomial((&lhs * &rhs)?)
                }
                (Function::Expression(_), _) | (_, Function::Expression(_)) => {
                    unreachable!("composite functions were handled above")
                }
            }
        };
        Ok(())
    }

    /// Multiply two borrowed functions. Products of non-constant compact
    /// polynomials are built into a fresh map, so neither complete operand map
    /// needs to be cloned.
    fn try_mul_refs(lhs: &Self, rhs: &Self) -> Result<Self, CoefficientError> {
        Ok(if !lhs.is_polynomial() || !rhs.is_polynomial() {
            match (lhs, rhs) {
                (lhs, Function::Constant(c)) if *c == Coefficient::one() => lhs.clone(),
                (Function::Constant(c), rhs) if *c == Coefficient::one() => rhs.clone(),
                (lhs, rhs) => {
                    associative_operation(AssociativeOperator::Mul, lhs.clone(), rhs.clone())
                }
            }
        } else {
            match (lhs, rhs) {
                (Function::Zero, _) | (_, Function::Zero) => Function::Zero,
                (Function::Constant(lhs), Function::Constant(rhs)) => {
                    if let Some(coefficient) = (*lhs * *rhs)? {
                        Function::Constant(coefficient)
                    } else {
                        Function::Zero
                    }
                }
                (Function::Constant(c), Function::Linear(l))
                | (Function::Linear(l), Function::Constant(c)) => Function::Linear((l * *c)?),
                (Function::Constant(c), Function::Quadratic(q))
                | (Function::Quadratic(q), Function::Constant(c)) => Function::Quadratic((q * *c)?),
                (Function::Constant(c), Function::Polynomial(p))
                | (Function::Polynomial(p), Function::Constant(c)) => {
                    Function::Polynomial((p * *c)?)
                }
                (Function::Linear(lhs), Function::Linear(rhs)) => Function::Quadratic((lhs * rhs)?),
                (Function::Linear(l), Function::Quadratic(q)) => Function::Polynomial((l * q)?),
                (Function::Quadratic(q), Function::Linear(l)) => Function::Polynomial((q * l)?),
                (Function::Linear(l), Function::Polynomial(p)) => Function::Polynomial((l * p)?),
                (Function::Polynomial(p), Function::Linear(l)) => Function::Polynomial((p * l)?),
                (Function::Quadratic(lhs), Function::Quadratic(rhs)) => {
                    Function::Polynomial((lhs * rhs)?)
                }
                (Function::Quadratic(q), Function::Polynomial(p)) => Function::Polynomial((q * p)?),
                (Function::Polynomial(p), Function::Quadratic(q)) => Function::Polynomial((p * q)?),
                (Function::Polynomial(lhs), Function::Polynomial(rhs)) => {
                    Function::Polynomial((lhs * rhs)?)
                }
                (Function::Expression(_), _) | (_, Function::Expression(_)) => {
                    unreachable!("composite functions were handled above")
                }
            }
        })
    }

    /// Multiply a borrowed left operand by an owned right operand while
    /// preserving the left-to-right accumulation order of polynomial products.
    fn try_mul_ref_owned(lhs: &Self, rhs: Self) -> Result<Self, CoefficientError> {
        Ok(if !lhs.is_polynomial() || !rhs.is_polynomial() {
            match (lhs, rhs) {
                (lhs, Function::Constant(c)) if c == Coefficient::one() => lhs.clone(),
                (Function::Constant(c), rhs) if *c == Coefficient::one() => rhs,
                (lhs, rhs) => associative_operation(AssociativeOperator::Mul, lhs.clone(), rhs),
            }
        } else {
            match (lhs, rhs) {
                (Function::Zero, _) | (_, Function::Zero) => Function::Zero,
                (Function::Constant(lhs), Function::Constant(rhs)) => {
                    if let Some(coefficient) = (*lhs * rhs)? {
                        Function::Constant(coefficient)
                    } else {
                        Function::Zero
                    }
                }
                (Function::Constant(c), Function::Linear(l)) => Function::Linear((l * *c)?),
                (Function::Linear(l), Function::Constant(c)) => Function::Linear((l * c)?),
                (Function::Constant(c), Function::Quadratic(q)) => Function::Quadratic((q * *c)?),
                (Function::Quadratic(q), Function::Constant(c)) => Function::Quadratic((q * c)?),
                (Function::Constant(c), Function::Polynomial(p)) => Function::Polynomial((p * *c)?),
                (Function::Polynomial(p), Function::Constant(c)) => Function::Polynomial((p * c)?),
                (Function::Linear(lhs), Function::Linear(rhs)) => {
                    Function::Quadratic((lhs * &rhs)?)
                }
                (Function::Linear(l), Function::Quadratic(q)) => Function::Polynomial((l * &q)?),
                (Function::Quadratic(q), Function::Linear(l)) => Function::Polynomial((q * &l)?),
                (Function::Linear(l), Function::Polynomial(p)) => Function::Polynomial((l * &p)?),
                (Function::Polynomial(p), Function::Linear(l)) => Function::Polynomial((p * &l)?),
                (Function::Quadratic(lhs), Function::Quadratic(rhs)) => {
                    Function::Polynomial((lhs * &rhs)?)
                }
                (Function::Quadratic(q), Function::Polynomial(p)) => {
                    Function::Polynomial((q * &p)?)
                }
                (Function::Polynomial(p), Function::Quadratic(q)) => {
                    Function::Polynomial((p * &q)?)
                }
                (Function::Polynomial(lhs), Function::Polynomial(rhs)) => {
                    Function::Polynomial((lhs * &rhs)?)
                }
                (Function::Expression(_), _) | (_, Function::Expression(_)) => {
                    unreachable!("composite functions were handled above")
                }
            }
        })
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
            lhs @ Function::Expression(_) => {
                associative_operation(AssociativeOperator::Mul, lhs, Function::Polynomial(rhs))
            }
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
            lhs @ Function::Expression(_) => {
                associative_operation(AssociativeOperator::Mul, lhs, Function::Linear(rhs.clone()))
            }
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
            lhs @ Function::Expression(_) => associative_operation(
                AssociativeOperator::Mul,
                lhs,
                Function::Quadratic(rhs.clone()),
            ),
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
            lhs @ Function::Expression(_) => associative_operation(
                AssociativeOperator::Mul,
                lhs,
                Function::Polynomial(rhs.clone()),
            ),
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
        Function::Constant(self) * rhs
    }
}

impl Mul<&Function> for Coefficient {
    type Output = Result<Function, CoefficientError>;

    fn mul(self, rhs: &Function) -> Self::Output {
        Function::Constant(self) * rhs
    }
}

impl Mul for Function {
    type Output = Result<Self, CoefficientError>;

    fn mul(self, rhs: Function) -> Self::Output {
        let mut out = self;
        out.try_mul_assign_owned_in_place(rhs)?;
        Ok(out.normalize())
    }
}

impl Mul for &Function {
    type Output = Result<Function, CoefficientError>;

    fn mul(self, rhs: Self) -> Self::Output {
        Ok(Function::try_mul_refs(self, rhs)?.normalize())
    }
}

impl Mul<Function> for &Function {
    type Output = Result<Function, CoefficientError>;

    fn mul(self, rhs: Function) -> Self::Output {
        Ok(Function::try_mul_ref_owned(self, rhs)?.normalize())
    }
}

impl Mul<&Function> for Function {
    type Output = Result<Function, CoefficientError>;

    fn mul(self, rhs: &Function) -> Self::Output {
        let mut out = self;
        out.try_mul_assign_in_place(rhs)?;
        Ok(out.normalize())
    }
}

macro_rules! impl_mul_polynomial_rhs {
    ($rhs:ty, $variant:ident, $ref_method:ident) => {
        impl Mul<$rhs> for Function {
            type Output = Result<Function, CoefficientError>;

            fn mul(self, rhs: $rhs) -> Self::Output {
                self * Function::$variant(rhs)
            }
        }

        impl Mul<$rhs> for &Function {
            type Output = Result<Function, CoefficientError>;

            fn mul(self, rhs: $rhs) -> Self::Output {
                self * Function::$variant(rhs)
            }
        }

        impl Mul<&$rhs> for Function {
            type Output = Result<Function, CoefficientError>;

            fn mul(self, rhs: &$rhs) -> Self::Output {
                let mut out = self;
                out.$ref_method(rhs)?;
                Ok(out.normalize())
            }
        }

        impl Mul<&$rhs> for &Function {
            type Output = Result<Function, CoefficientError>;

            fn mul(self, rhs: &$rhs) -> Self::Output {
                let mut out = self.clone();
                out.$ref_method(rhs)?;
                Ok(out.normalize())
            }
        }
    };
}

impl_mul_polynomial_rhs!(Linear, Linear, try_mul_linear_assign_in_place);
impl_mul_polynomial_rhs!(Quadratic, Quadratic, try_mul_quadratic_assign_in_place);
impl_mul_polynomial_rhs!(
    Polynomial,
    Polynomial,
    try_mul_polynomial_ref_assign_in_place
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, linear, monomial, FunctionParameters, MonomialDyn, PolynomialParameters};
    use ::approx::assert_abs_diff_eq;
    use proptest::prelude::*;

    fn polynomial_function() -> BoxedStrategy<Function> {
        Function::arbitrary_with(FunctionParameters::polynomial_only(
            PolynomialParameters::default(),
        ))
    }

    proptest! {
        #[test]
        fn mul_ref(a in any::<Function>(), b in any::<Function>()) {
            let ans = (a.clone() * b.clone()).unwrap();
            assert_abs_diff_eq!((&a * &b).unwrap(), ans);
            assert_abs_diff_eq!((a.clone() * &b).unwrap(), ans);
            assert_abs_diff_eq!((&a * b).unwrap(), ans);
        }

        #[test]
        fn zero(a in polynomial_function()) {
            assert_abs_diff_eq!((&a * Function::zero()).unwrap(), Function::zero());
            assert_abs_diff_eq!((Function::zero() * &a).unwrap(), Function::zero());
        }

        #[test]
        fn mul_commutative(a in polynomial_function(), b in polynomial_function()) {
            assert_abs_diff_eq!((&a * &b).unwrap(), (&b * &a).unwrap());
        }

        #[test]
        fn mul_nary(
            a in polynomial_function(),
            b in polynomial_function(),
            c in polynomial_function(),
        ) {
            assert_abs_diff_eq!((&a * (&b * &c).unwrap()).unwrap(), ((&a * &b).unwrap() * &c).unwrap());
        }
    }

    #[test]
    fn mixed_ownership_preserves_polynomial_accumulation_order() {
        let lhs = Function::Polynomial(
            Polynomial::try_from_terms([
                (MonomialDyn::default(), coeff!(1.0)),
                (monomial!(1), coeff!(1.0)),
                (monomial!(1, 1), coeff!(1.0)),
            ])
            .unwrap(),
        );
        let rhs = Function::Polynomial(
            Polynomial::try_from_terms([
                (MonomialDyn::default(), coeff!(1.0)),
                (monomial!(1), coeff!(-1e16)),
                (monomial!(1, 1), coeff!(1e16)),
            ])
            .unwrap(),
        );

        let expected = (lhs.clone() * rhs.clone()).unwrap();
        let reversed = (rhs.clone() * &lhs).unwrap();
        assert!(
            expected != reversed,
            "fixture must expose accumulation order"
        );
        assert!((&lhs * rhs.clone()).unwrap() == expected);
        assert!((lhs.clone() * &rhs).unwrap() == expected);
        assert!((&lhs * &rhs).unwrap() == expected);
    }

    #[test]
    fn borrowed_multiplication_preserves_coefficient_error() {
        let huge = Function::Linear(Linear::single_term(linear!(1), coeff!(f64::MAX)));
        assert!(matches!(&huge * &huge, Err(CoefficientError::Infinite)));
    }
}
