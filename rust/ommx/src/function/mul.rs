use super::*;
use crate::CoefficientError;
use std::ops::Mul;

impl Function {
    pub fn one() -> Self {
        Function::Constant(Coefficient::one())
    }
}

impl Mul<Coefficient> for Function {
    type Output = Result<Self, CoefficientError>;

    fn mul(self, rhs: Coefficient) -> Self::Output {
        Ok(Function::from_polynomial((self.into_polynomial() * rhs)?))
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
        Ok(Function::from_polynomial(
            (self.into_polynomial() * rhs.into_polynomial())?,
        ))
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
                Ok(Function::from_polynomial(
                    (self.into_polynomial() * Function::from(rhs.clone()).into_polynomial())?,
                ))
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
