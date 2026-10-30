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
            (lhs, rhs) => {
                let mut out = lhs;
                out.try_add_assign_in_place(-rhs.clone())?;
                out
            }
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
                let mut out = lhs.clone();
                out.try_add_assign_in_place(-rhs.clone())?;
                out
            }
        })
    }

    fn try_sub_linear_ref(self, rhs: &Linear) -> Result<Self, CoefficientError> {
        Ok(match self {
            Function::Zero => Function::Linear(-rhs.clone()),
            Function::Constant(c) => Function::Linear((c - rhs)?),
            Function::Linear(lhs) => Function::Linear((lhs - rhs)?),
            Function::Quadratic(lhs) => Function::Quadratic((lhs - rhs)?),
            Function::Polynomial(lhs) => Function::Polynomial((lhs - rhs)?),
            lhs @ Function::Expression(_) => associative_operation(
                AssociativeOperator::Add,
                lhs,
                -Function::Linear(rhs.clone()),
            ),
        })
    }

    fn try_sub_quadratic_ref(self, rhs: &Quadratic) -> Result<Self, CoefficientError> {
        Ok(match self {
            Function::Zero => Function::Quadratic(-rhs.clone()),
            Function::Constant(c) => Function::Quadratic((c - rhs)?),
            Function::Linear(lhs) => Function::Quadratic(((-rhs.clone()) + &lhs)?),
            Function::Quadratic(lhs) => Function::Quadratic((lhs - rhs)?),
            Function::Polynomial(lhs) => Function::Polynomial((lhs - rhs)?),
            lhs @ Function::Expression(_) => associative_operation(
                AssociativeOperator::Add,
                lhs,
                -Function::Quadratic(rhs.clone()),
            ),
        })
    }

    fn try_sub_polynomial_ref(self, rhs: &Polynomial) -> Result<Self, CoefficientError> {
        Ok(match self {
            Function::Zero => Function::Polynomial(-rhs.clone()),
            Function::Constant(c) => Function::Polynomial((c - rhs)?),
            Function::Linear(lhs) => Function::Polynomial(((-rhs.clone()) + &lhs)?),
            Function::Quadratic(lhs) => Function::Polynomial(((-rhs.clone()) + &lhs)?),
            Function::Polynomial(lhs) => Function::Polynomial((lhs - rhs)?),
            lhs @ Function::Expression(_) => associative_operation(
                AssociativeOperator::Add,
                lhs,
                -Function::Polynomial(rhs.clone()),
            ),
        })
    }

    fn try_sub_linear_refs(lhs: &Self, rhs: &Linear) -> Result<Self, CoefficientError> {
        Ok(match lhs {
            Function::Zero => Function::Linear(-rhs.clone()),
            Function::Constant(c) => Function::Linear((*c - rhs)?),
            Function::Linear(lhs) => Function::Linear((lhs - rhs)?),
            Function::Quadratic(lhs) => Function::Quadratic((lhs.clone() - rhs)?),
            Function::Polynomial(lhs) => Function::Polynomial((lhs.clone() - rhs)?),
            lhs @ Function::Expression(_) => associative_operation(
                AssociativeOperator::Add,
                lhs.clone(),
                -Function::Linear(rhs.clone()),
            ),
        })
    }

    fn try_sub_quadratic_refs(lhs: &Self, rhs: &Quadratic) -> Result<Self, CoefficientError> {
        Ok(match lhs {
            Function::Zero => Function::Quadratic(-rhs.clone()),
            Function::Constant(c) => Function::Quadratic((*c - rhs)?),
            Function::Linear(lhs) => Function::Quadratic(((-rhs.clone()) + lhs)?),
            Function::Quadratic(lhs) => Function::Quadratic((lhs - rhs)?),
            Function::Polynomial(lhs) => Function::Polynomial((lhs.clone() - rhs)?),
            lhs @ Function::Expression(_) => associative_operation(
                AssociativeOperator::Add,
                lhs.clone(),
                -Function::Quadratic(rhs.clone()),
            ),
        })
    }

    fn try_sub_polynomial_refs(lhs: &Self, rhs: &Polynomial) -> Result<Self, CoefficientError> {
        Ok(match lhs {
            Function::Zero => Function::Polynomial(-rhs.clone()),
            Function::Constant(c) => Function::Polynomial((*c - rhs)?),
            Function::Linear(lhs) => Function::Polynomial(((-rhs.clone()) + lhs)?),
            Function::Quadratic(lhs) => Function::Polynomial(((-rhs.clone()) + lhs)?),
            Function::Polynomial(lhs) => Function::Polynomial((lhs - rhs)?),
            lhs @ Function::Expression(_) => associative_operation(
                AssociativeOperator::Add,
                lhs.clone(),
                -Function::Polynomial(rhs.clone()),
            ),
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
    ($rhs:ty, $owned_ref_method:ident, $refs_method:ident) => {
        impl Sub<&$rhs> for Function {
            type Output = Result<Function, CoefficientError>;

            fn sub(self, rhs: &$rhs) -> Self::Output {
                Ok(self.$owned_ref_method(rhs)?.normalize())
            }
        }

        impl Sub<&$rhs> for &Function {
            type Output = Result<Function, CoefficientError>;

            fn sub(self, rhs: &$rhs) -> Self::Output {
                Ok(Function::$refs_method(self, rhs)?.normalize())
            }
        }

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

impl_sub_polynomial_rhs!(Linear, try_sub_linear_ref, try_sub_linear_refs);
impl_sub_polynomial_rhs!(Quadratic, try_sub_quadratic_ref, try_sub_quadratic_refs);
impl_sub_polynomial_rhs!(Polynomial, try_sub_polynomial_ref, try_sub_polynomial_refs);

#[cfg(test)]
mod tests {
    use super::super::operation::{unary_expression, UnaryOperator};
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

        #[test]
        fn fixed_rhs_refs_match_function_dispatch(
            lhs in any::<Function>(),
            linear in any::<Linear>(),
            quadratic in any::<Quadratic>(),
            polynomial in any::<Polynomial>(),
        ) {
            prop_assert_eq!(lhs.clone() - &linear, lhs.clone() - Function::Linear(linear.clone()));
            prop_assert_eq!(&lhs - &linear, &lhs - Function::Linear(linear));
            prop_assert_eq!(lhs.clone() - &quadratic, lhs.clone() - Function::Quadratic(quadratic.clone()));
            prop_assert_eq!(&lhs - &quadratic, &lhs - Function::Quadratic(quadratic));
            prop_assert_eq!(lhs.clone() - &polynomial, lhs.clone() - Function::Polynomial(polynomial.clone()));
            prop_assert_eq!(&lhs - &polynomial, &lhs - Function::Polynomial(polynomial));
        }
    }

    #[test]
    fn borrowed_subtraction_redispatches_after_double_negation() {
        let lhs = Function::from(linear!(1));
        let rhs = unary_expression(UnaryOperator::Neg, Function::from(linear!(2)));
        let expected = Function::from((linear!(1) + linear!(2)).unwrap());
        assert!(matches!(&rhs, Function::Expression(_)));

        for actual in [
            (lhs.clone() - rhs.clone()).unwrap(),
            (&lhs - &rhs).unwrap(),
            (&lhs - rhs.clone()).unwrap(),
            (lhs.clone() - &rhs).unwrap(),
        ] {
            assert!(actual.is_polynomial());
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn borrowed_subtraction_preserves_double_negation_coefficient_error() {
        let huge = Function::Constant(coeff!(f64::MAX));
        let negative_huge = unary_expression(UnaryOperator::Neg, huge.clone());

        assert!(matches!(
            huge.clone() - negative_huge.clone(),
            Err(CoefficientError::Infinite)
        ));
        assert!(matches!(
            &huge - &negative_huge,
            Err(CoefficientError::Infinite)
        ));
        assert!(matches!(
            &huge - negative_huge.clone(),
            Err(CoefficientError::Infinite)
        ));
        assert!(matches!(
            huge - &negative_huge,
            Err(CoefficientError::Infinite)
        ));
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

    #[test]
    fn fixed_borrowed_subtraction_preserves_expression_order_and_coefficient_error() {
        let lhs = Function::from(linear!(1)).abs();
        let rhs = Linear::from(linear!(2));
        let expected = (lhs.clone() - Function::Linear(rhs.clone())).unwrap();

        assert!((lhs.clone() - &rhs).unwrap() == expected);
        assert!((&lhs - &rhs).unwrap() == expected);

        let huge = Function::Linear(Linear::single_term(linear!(1), coeff!(f64::MAX)));
        let negative_huge_rhs = Linear::single_term(linear!(1), coeff!(-f64::MAX));
        assert!(matches!(
            huge.clone() - &negative_huge_rhs,
            Err(CoefficientError::Infinite)
        ));
        assert!(matches!(
            &huge - &negative_huge_rhs,
            Err(CoefficientError::Infinite)
        ));
    }
}
