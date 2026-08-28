use std::ops::Div;

use super::*;
use crate::CoefficientError;

impl Div<Coefficient> for Function {
    type Output = Result<Self, CoefficientError>;

    fn div(mut self, rhs: Coefficient) -> Self::Output {
        match &mut self {
            Function::Zero => {}
            Function::Constant(c) => {
                if let Some(divided) = (*c / rhs)? {
                    *c = divided;
                } else {
                    self = Function::Zero;
                }
            }
            Function::Linear(l) => l.try_div_assign_in_place(rhs)?,
            Function::Quadratic(q) => q.try_div_assign_in_place(rhs)?,
            Function::Polynomial(p) => p.try_div_assign_in_place(rhs)?,
        }
        // Division can underflow coefficients to zero and remove terms, so
        // the variant may need to be downgraded.
        Ok(self.normalize())
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
                let c_ref = &c;
                assert_abs_diff_eq!((&a / c).unwrap(), expected);
                assert_abs_diff_eq!((a.clone() / c_ref).unwrap(), expected);
                assert_abs_diff_eq!((&a / c_ref).unwrap(), expected);
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
    fn div_directly_divides_coefficients() {
        let tiny = Coefficient::try_from(f64::from_bits(1)).unwrap();
        assert_abs_diff_eq!(
            (Function::from(tiny) / tiny).unwrap(),
            Function::from(Coefficient::one())
        );
    }
}
