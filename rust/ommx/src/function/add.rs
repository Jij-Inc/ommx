use super::*;
use crate::{CoefficientError, MonomialDyn};
use std::ops::{Add, Neg};

impl Function {
    pub fn zero() -> Self {
        Function::Zero
    }

    pub fn is_zero(&self) -> bool {
        matches!(self, Function::Zero)
    }

    pub(crate) fn into_polynomial(self) -> Polynomial {
        match self {
            Function::Zero => Polynomial::zero(),
            Function::Constant(c) => Polynomial::from(c),
            Function::Linear(l) => l
                .into_iter()
                .map(|(m, c)| (MonomialDyn::from(m), c))
                .collect(),
            Function::Quadratic(q) => q
                .into_iter()
                .map(|(m, c)| (MonomialDyn::from(m), c))
                .collect(),
            Function::Polynomial(p) => p,
        }
    }
}

impl Add for Function {
    type Output = Result<Self, CoefficientError>;

    fn add(self, rhs: Self) -> Self::Output {
        Ok(Function::from(
            (self.into_polynomial() + rhs.into_polynomial())?,
        ))
    }
}

impl Add for &Function {
    type Output = Result<Function, CoefficientError>;

    fn add(self, rhs: Self) -> Self::Output {
        self.clone() + rhs.clone()
    }
}

impl Add<Function> for &Function {
    type Output = Result<Function, CoefficientError>;

    fn add(self, rhs: Function) -> Self::Output {
        self.clone() + rhs
    }
}

impl Add<&Function> for Function {
    type Output = Result<Function, CoefficientError>;

    fn add(self, rhs: &Function) -> Self::Output {
        self + rhs.clone()
    }
}

impl Add<Coefficient> for Function {
    type Output = Result<Self, CoefficientError>;

    fn add(self, rhs: Coefficient) -> Self::Output {
        Ok(Function::from((self.into_polynomial() + rhs)?))
    }
}

impl Add<Coefficient> for &Function {
    type Output = Result<Function, CoefficientError>;

    fn add(self, rhs: Coefficient) -> Self::Output {
        self.clone() + rhs
    }
}

impl Add<&Coefficient> for Function {
    type Output = Result<Self, CoefficientError>;

    fn add(self, rhs: &Coefficient) -> Self::Output {
        self + *rhs
    }
}

impl Add<&Coefficient> for &Function {
    type Output = Result<Function, CoefficientError>;

    fn add(self, rhs: &Coefficient) -> Self::Output {
        self.clone() + *rhs
    }
}

impl Add<Function> for Coefficient {
    type Output = Result<Function, CoefficientError>;

    fn add(self, rhs: Function) -> Self::Output {
        rhs + self
    }
}

impl Add<&Function> for Coefficient {
    type Output = Result<Function, CoefficientError>;

    fn add(self, rhs: &Function) -> Self::Output {
        rhs.clone() + self
    }
}

macro_rules! impl_add_polynomial_rhs {
    ($rhs:ty) => {
        impl Add<$rhs> for Function {
            type Output = Result<Function, CoefficientError>;

            fn add(self, rhs: $rhs) -> Self::Output {
                Ok(Function::from(
                    (self.into_polynomial() + Function::from(rhs.clone()).into_polynomial())?,
                ))
            }
        }

        impl Add<$rhs> for &Function {
            type Output = Result<Function, CoefficientError>;

            fn add(self, rhs: $rhs) -> Self::Output {
                self.clone() + rhs
            }
        }
    };
}

impl_add_polynomial_rhs!(Linear);
impl_add_polynomial_rhs!(&Linear);
impl_add_polynomial_rhs!(Quadratic);
impl_add_polynomial_rhs!(&Quadratic);
impl_add_polynomial_rhs!(Polynomial);
impl_add_polynomial_rhs!(&Polynomial);

impl Neg for Function {
    type Output = Self;

    fn neg(mut self) -> Self::Output {
        self.values_mut().for_each(|v| *v = -(*v));
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::approx::assert_abs_diff_eq;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn add_ref(a in any::<Function>(), b in any::<Function>()) {
            let ans = (a.clone() + b.clone()).unwrap();
            assert_abs_diff_eq!((&a + &b).unwrap(), ans);
            assert_abs_diff_eq!((&a + b.clone()).unwrap(), ans);
            assert_abs_diff_eq!((a + &b).unwrap(), ans);
        }

        #[test]
        fn zero(a in any::<Function>()) {
            assert_abs_diff_eq!((&a + Function::zero()).unwrap(), a.clone());
            assert_abs_diff_eq!((Function::zero() + &a).unwrap(), a.clone());
        }

        #[test]
        fn add_commutative(a in any::<Function>(), b in any::<Function>()) {
            assert_abs_diff_eq!((&a + &b).unwrap(), (&b + &a).unwrap());
        }

        #[test]
        fn add_associative(a in any::<Function>(), b in any::<Function>(), c in any::<Function>()) {
            assert_abs_diff_eq!((&a + (&b + &c).unwrap()).unwrap(), ((&a + &b).unwrap() + &c).unwrap());
        }
    }
}
