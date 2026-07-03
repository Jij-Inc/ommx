use super::*;
use crate::CoefficientError;
use std::ops::Sub;

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
        self.clone() - rhs.clone()
    }
}

impl Sub<Function> for &Function {
    type Output = Result<Function, CoefficientError>;

    fn sub(self, rhs: Function) -> Self::Output {
        self.clone() - rhs
    }
}

impl Sub<&Function> for Function {
    type Output = Result<Function, CoefficientError>;

    fn sub(self, rhs: &Function) -> Self::Output {
        self - rhs.clone()
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
        self.clone() - rhs
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
        self.clone() - *rhs
    }
}

impl Sub<Function> for Coefficient {
    type Output = Result<Function, CoefficientError>;

    fn sub(self, rhs: Function) -> Self::Output {
        (-rhs) + self
    }
}

impl Sub<&Function> for Coefficient {
    type Output = Result<Function, CoefficientError>;

    fn sub(self, rhs: &Function) -> Self::Output {
        self - rhs.clone()
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
                self.clone() - rhs
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
                self.clone() - rhs
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
    use ::approx::assert_abs_diff_eq;
    use proptest::prelude::*;

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
        fn neg_sub(a in any::<Function>(), b in any::<Function>()) {
            assert_abs_diff_eq!(-(a.clone() - b.clone()).unwrap(), (b - a).unwrap());
        }
    }
}
