use num::traits::Inv;
use std::ops::Div;

use super::*;
use crate::CoefficientError;

impl Div<Coefficient> for Function {
    type Output = Result<Self, CoefficientError>;

    fn div(self, rhs: Coefficient) -> Self::Output {
        self * rhs.inv()?
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
    use ::approx::assert_abs_diff_eq;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn div_ref(a in any::<Function>(), c in any::<Coefficient>()) {
            if let Ok(expected) = a.clone() / c {
                assert_abs_diff_eq!((&a / c).unwrap(), expected);
                assert_abs_diff_eq!((a.clone() / &c).unwrap(), expected);
                assert_abs_diff_eq!((&a / &c).unwrap(), expected);
            }
        }

        #[test]
        fn zero(a in any::<Coefficient>()) {
            if let Ok(divided) = Function::zero() / a {
                assert_abs_diff_eq!(divided, Function::zero());
            }
        }
    }

    #[test]
    fn div_uses_fallible_reciprocal() {
        let tiny = Coefficient::try_from(f64::from_bits(1)).unwrap();
        assert!(matches!(
            Function::from(tiny) / tiny,
            Err(CoefficientError::Infinite)
        ));
    }
}
