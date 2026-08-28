use super::operation::{associative_operation, AssociativeOperator};
use super::*;
use crate::CoefficientError;
use std::ops::Sub;

impl Function {
    /// Subtract a borrowed function without first cloning it into a `Function`.
    fn try_sub_assign_ref_in_place(&mut self, rhs: &Self) -> Result<(), CoefficientError> {
        let lhs = std::mem::take(self);
        *self = match (lhs, rhs) {
            (lhs, Function::Zero) => lhs,
            (Function::Zero, rhs) => -rhs.clone(),
            (Function::Constant(lhs), Function::Constant(rhs)) => {
                if let Some(coefficient) = (lhs - *rhs)? {
                    Function::Constant(coefficient)
                } else {
                    Function::Zero
                }
            }
            (Function::Constant(c), Function::Linear(l)) => Function::Linear((c - l)?),
            (Function::Linear(l), Function::Constant(c)) => Function::Linear((l - *c)?),
            (Function::Constant(c), Function::Quadratic(q)) => Function::Quadratic((c - q)?),
            (Function::Quadratic(q), Function::Constant(c)) => Function::Quadratic((q - *c)?),
            (Function::Constant(c), Function::Polynomial(p)) => Function::Polynomial((c - p)?),
            (Function::Polynomial(p), Function::Constant(c)) => Function::Polynomial((p - *c)?),
            (Function::Linear(lhs), Function::Linear(rhs)) => Function::Linear((lhs - rhs)?),
            (Function::Linear(l), Function::Quadratic(q)) => {
                Function::Quadratic(((-q.clone()) + &l)?)
            }
            (Function::Quadratic(q), Function::Linear(l)) => Function::Quadratic((q - l)?),
            (Function::Linear(l), Function::Polynomial(p)) => {
                Function::Polynomial(((-p.clone()) + &l)?)
            }
            (Function::Polynomial(p), Function::Linear(l)) => Function::Polynomial((p - l)?),
            (Function::Quadratic(lhs), Function::Quadratic(rhs)) => {
                Function::Quadratic((lhs - rhs)?)
            }
            (Function::Quadratic(q), Function::Polynomial(p)) => {
                Function::Polynomial(((-p.clone()) + &q)?)
            }
            (Function::Polynomial(p), Function::Quadratic(q)) => Function::Polynomial((p - q)?),
            (Function::Polynomial(lhs), Function::Polynomial(rhs)) => {
                Function::Polynomial((lhs - rhs)?)
            }
            (lhs, rhs) => associative_operation(AssociativeOperator::Add, lhs, -rhs.clone()),
        };
        Ok(())
    }

    /// Subtract two borrowed functions while cloning only storage that must be
    /// owned by the result.
    fn try_sub_refs(lhs: &Self, rhs: &Self) -> Result<Self, CoefficientError> {
        Ok(match (lhs, rhs) {
            (lhs, Function::Zero) => lhs.clone(),
            (Function::Zero, rhs) => -rhs.clone(),
            (Function::Constant(lhs), Function::Constant(rhs)) => {
                if let Some(coefficient) = (*lhs - *rhs)? {
                    Function::Constant(coefficient)
                } else {
                    Function::Zero
                }
            }
            (Function::Constant(c), Function::Linear(l)) => Function::Linear((*c - l)?),
            (Function::Linear(l), Function::Constant(c)) => Function::Linear((l - *c)?),
            (Function::Constant(c), Function::Quadratic(q)) => Function::Quadratic((*c - q)?),
            (Function::Quadratic(q), Function::Constant(c)) => Function::Quadratic((q - *c)?),
            (Function::Constant(c), Function::Polynomial(p)) => Function::Polynomial((*c - p)?),
            (Function::Polynomial(p), Function::Constant(c)) => Function::Polynomial((p - *c)?),
            (Function::Linear(lhs), Function::Linear(rhs)) => Function::Linear((lhs - rhs)?),
            (Function::Linear(l), Function::Quadratic(q)) => {
                Function::Quadratic(((-q.clone()) + l)?)
            }
            (Function::Quadratic(q), Function::Linear(l)) => Function::Quadratic((q.clone() - l)?),
            (Function::Linear(l), Function::Polynomial(p)) => {
                Function::Polynomial(((-p.clone()) + l)?)
            }
            (Function::Polynomial(p), Function::Linear(l)) => {
                Function::Polynomial((p.clone() - l)?)
            }
            (Function::Quadratic(lhs), Function::Quadratic(rhs)) => {
                Function::Quadratic((lhs - rhs)?)
            }
            (Function::Quadratic(q), Function::Polynomial(p)) => {
                Function::Polynomial(((-p.clone()) + q)?)
            }
            (Function::Polynomial(p), Function::Quadratic(q)) => {
                Function::Polynomial((p.clone() - q)?)
            }
            (Function::Polynomial(lhs), Function::Polynomial(rhs)) => {
                Function::Polynomial((lhs - rhs)?)
            }
            (lhs, rhs) => {
                associative_operation(AssociativeOperator::Add, lhs.clone(), -rhs.clone())
            }
        })
    }
}

impl Sub for Function {
    type Output = Result<Self, CoefficientError>;

    fn sub(mut self, rhs: Self) -> Self::Output {
        // `x - y = x + (-y)` term by term, so this matches a term-level
        // subtraction exactly (`Sub` on the term maps also adds `-c`).
        self.try_add_assign_in_place(-rhs)?;
        Ok(self.normalize())
    }
}

impl Sub for &Function {
    type Output = Result<Function, CoefficientError>;

    fn sub(self, rhs: Self) -> Self::Output {
        Ok(Function::try_sub_refs(self, rhs)?.normalize())
    }
}

impl Sub<Function> for &Function {
    type Output = Result<Function, CoefficientError>;

    fn sub(self, rhs: Function) -> Self::Output {
        if self.is_polynomial() && rhs.is_polynomial() {
            // For compact functions, `lhs - rhs = -rhs + lhs` lets the owned
            // right-hand map carry the result without cloning both operands.
            (-rhs) + self
        } else {
            self.clone() - rhs
        }
    }
}

impl Sub<&Function> for Function {
    type Output = Result<Function, CoefficientError>;

    fn sub(self, rhs: &Function) -> Self::Output {
        let mut out = self;
        out.try_sub_assign_ref_in_place(rhs)?;
        Ok(out.normalize())
    }
}

impl Sub<Coefficient> for Function {
    type Output = Result<Self, CoefficientError>;

    fn sub(mut self, rhs: Coefficient) -> Self::Output {
        self.try_add_assign_in_place(Function::Constant(-rhs))?;
        Ok(self.normalize())
    }
}

impl Sub<Coefficient> for &Function {
    type Output = Result<Function, CoefficientError>;

    fn sub(self, rhs: Coefficient) -> Self::Output {
        self - Function::Constant(rhs)
    }
}

impl Sub<&Coefficient> for Function {
    type Output = Result<Self, CoefficientError>;

    fn sub(self, rhs: &Coefficient) -> Self::Output {
        self - *rhs
    }
}

impl Sub<&Coefficient> for &Function {
    type Output = Result<Function, CoefficientError>;

    fn sub(self, rhs: &Coefficient) -> Self::Output {
        self - Function::Constant(*rhs)
    }
}

impl Sub<Function> for Coefficient {
    type Output = Result<Function, CoefficientError>;

    fn sub(self, rhs: Function) -> Self::Output {
        Function::Constant(self) + (-rhs)
    }
}

impl Sub<&Function> for Coefficient {
    type Output = Result<Function, CoefficientError>;

    fn sub(self, rhs: &Function) -> Self::Output {
        Function::Constant(self) - rhs
    }
}

macro_rules! impl_sub_polynomial_rhs {
    (& $rhs:ty) => {
        impl Sub<&$rhs> for Function {
            type Output = Result<Function, CoefficientError>;

            fn sub(mut self, rhs: &$rhs) -> Self::Output {
                self.try_add_assign_in_place(-Function::from(rhs.clone()))?;
                Ok(self.normalize())
            }
        }

        impl Sub<&$rhs> for &Function {
            type Output = Result<Function, CoefficientError>;

            fn sub(self, rhs: &$rhs) -> Self::Output {
                self - Function::from(rhs.clone())
            }
        }
    };
    ($rhs:ty) => {
        impl Sub<$rhs> for Function {
            type Output = Result<Function, CoefficientError>;

            fn sub(mut self, rhs: $rhs) -> Self::Output {
                self.try_add_assign_in_place(-Function::from(rhs))?;
                Ok(self.normalize())
            }
        }

        impl Sub<$rhs> for &Function {
            type Output = Result<Function, CoefficientError>;

            fn sub(self, rhs: $rhs) -> Self::Output {
                self - Function::from(rhs)
            }
        }
    };
}

impl_sub_polynomial_rhs!(Linear);
impl_sub_polynomial_rhs!(&Linear);
impl_sub_polynomial_rhs!(Quadratic);
impl_sub_polynomial_rhs!(&Quadratic);
impl_sub_polynomial_rhs!(Polynomial);
impl_sub_polynomial_rhs!(&Polynomial);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, linear, FunctionParameters, PolynomialParameters};
    use ::approx::assert_abs_diff_eq;
    use proptest::prelude::*;

    fn polynomial_function() -> BoxedStrategy<Function> {
        Function::arbitrary_with(FunctionParameters::polynomial_only(
            PolynomialParameters::default(),
        ))
    }

    proptest! {
        #[test]
        fn sub_ref(a in any::<Function>(), b in any::<Function>()) {
            let expected = (a.clone() - b.clone()).unwrap();
            assert_abs_diff_eq!((&a - &b).unwrap(), expected);
            assert_abs_diff_eq!((&a - b.clone()).unwrap(), expected);
            assert_abs_diff_eq!((a.clone() - &b).unwrap(), expected);
        }

        #[test]
        fn zero_sub(a in any::<Function>()) {
            assert_abs_diff_eq!((&a - Function::zero()).unwrap(), a.clone());
            assert_abs_diff_eq!((Function::zero() - &a).unwrap(), -a.clone());
        }

        #[test]
        fn sub_via_add_neg(a in any::<Function>(), b in any::<Function>()) {
            assert_abs_diff_eq!((a.clone() - b.clone()).unwrap(), (a + (-b.clone())).unwrap());
        }

        #[test]
        fn neg_sub(a in polynomial_function(), b in polynomial_function()) {
            assert_abs_diff_eq!(-(a.clone() - b.clone()).unwrap(), (b - a).unwrap());
        }
    }

    #[test]
    fn borrowed_subtraction_preserves_coefficient_error() {
        let huge = Function::Linear(Linear::single_term(linear!(1), coeff!(f64::MAX)));
        let negative_huge = -huge.clone();
        assert!(matches!(
            &huge - &negative_huge,
            Err(CoefficientError::Infinite)
        ));
    }
}
