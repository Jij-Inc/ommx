use std::ops::Div;

use super::operation::{binary_operation, BinaryOperator};
use super::*;
use crate::CoefficientError;

impl Div<Coefficient> for Function {
    type Output = Result<Self, CoefficientError>;

    fn div(self, rhs: Coefficient) -> Self::Output {
        Ok(binary_operation(
            BinaryOperator::Div,
            self,
            Function::Constant(rhs),
        ))
    }
}

impl Div for Function {
    type Output = Result<Self, CoefficientError>;

    fn div(self, rhs: Self) -> Self::Output {
        Ok(binary_operation(BinaryOperator::Div, self, rhs))
    }
}

impl Div for &Function {
    type Output = Result<Function, CoefficientError>;

    fn div(self, rhs: Self) -> Self::Output {
        self.clone() / rhs.clone()
    }
}

impl Div<Function> for &Function {
    type Output = Result<Function, CoefficientError>;

    fn div(self, rhs: Function) -> Self::Output {
        self.clone() / rhs
    }
}

impl Div<&Function> for Function {
    type Output = Result<Function, CoefficientError>;

    fn div(self, rhs: &Function) -> Self::Output {
        self / rhs.clone()
    }
}

impl Div<&Coefficient> for Function {
    type Output = Result<Self, CoefficientError>;

    fn div(self, rhs: &Coefficient) -> Self::Output {
        self / *rhs
    }
}

impl Div<Coefficient> for &Function {
    type Output = Result<Function, CoefficientError>;

    fn div(self, rhs: Coefficient) -> Self::Output {
        self.clone() / rhs
    }
}

impl Div<&Coefficient> for &Function {
    type Output = Result<Function, CoefficientError>;

    fn div(self, rhs: &Coefficient) -> Self::Output {
        self.clone() / *rhs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ATol, Evaluate, FunctionEvaluationError};
    use ::approx::assert_abs_diff_eq;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn div_ref(a in any::<Function>(), c in any::<Coefficient>()) {
            if let Ok(expected) = a.clone() / c {
                let c_ref = &c;
                assert_abs_diff_eq!((&a / c).unwrap(), expected);
                assert_abs_diff_eq!((a.clone() / c_ref).unwrap(), expected);
                assert_abs_diff_eq!((&a / c_ref).unwrap(), expected);
            }
        }

        #[test]
        fn zero(a in any::<Coefficient>()) {
            let divided = (Function::zero() / a).unwrap();
            prop_assert!(matches!(divided, Function::Expression(_)));
        }
    }

    #[test]
    fn function_division_defers_tolerance_sensitive_zero_classification() {
        let tiny = Coefficient::try_from(f64::from_bits(1)).unwrap();
        let divided = (Function::from(tiny) / tiny).unwrap();
        assert!(matches!(&divided, Function::Expression(_)));

        let error = divided
            .evaluate(
                &crate::v1::State::default(),
                ATol::new(f64::from_bits(1)).unwrap(),
            )
            .unwrap_err();
        assert!(matches!(
            error.downcast_ref::<FunctionEvaluationError>(),
            Some(FunctionEvaluationError::DivisionByZero)
        ));
    }
}
