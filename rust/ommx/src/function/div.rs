use std::ops::Div;

use super::*;
use crate::CoefficientError;

fn divide_polynomial(
    polynomial: Polynomial,
    rhs: Coefficient,
) -> Result<Polynomial, CoefficientError> {
    let mut out = Polynomial::zero();
    for (monomial, coefficient) in polynomial.iter() {
        if let Some(coefficient) = (*coefficient / rhs)? {
            out.add_term(monomial.clone(), coefficient)?;
        }
    }
    Ok(out)
}

impl Div<Coefficient> for Function {
    type Output = Result<Self, CoefficientError>;

    fn div(self, rhs: Coefficient) -> Self::Output {
        Ok(Function::from_polynomial(divide_polynomial(
            self.into_polynomial(),
            rhs,
        )?))
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
