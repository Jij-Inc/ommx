use super::*;
use crate::{CoefficientError, LinearMonomial, MonomialDyn, QuadraticMonomial};
use std::ops::{Add, Neg};

impl Function {
    pub fn zero() -> Self {
        Function::Zero
    }

    pub fn is_zero(&self) -> bool {
        matches!(self, Function::Zero)
    }

    pub(crate) fn from_polynomial(polynomial: Polynomial) -> Self {
        if polynomial.is_zero() {
            Function::Zero
        } else if polynomial.degree() == 0 {
            Function::Constant(
                polynomial
                    .get(&MonomialDyn::default())
                    .expect("non-zero degree-0 polynomial has a constant term"),
            )
        } else if let Ok(linear) = Linear::try_from(&polynomial) {
            Function::Linear(linear)
        } else if let Ok(quadratic) = Quadratic::try_from(&polynomial) {
            Function::Quadratic(quadratic)
        } else {
            Function::Polynomial(polynomial)
        }
    }

    pub(crate) fn into_polynomial(self) -> Polynomial {
        match self {
            Function::Zero => Polynomial::zero(),
            Function::Constant(c) => Polynomial::from(c),
            Function::Linear(l) => Polynomial::new(
                l.into_iter()
                    .map(|(m, c)| (MonomialDyn::from(m), c))
                    .collect(),
            ),
            Function::Quadratic(q) => Polynomial::new(
                q.into_iter()
                    .map(|(m, c)| (MonomialDyn::from(m), c))
                    .collect(),
            ),
            Function::Polynomial(p) => p,
        }
    }

    pub(crate) fn normalize(self) -> Self {
        Function::from_polynomial(self.into_polynomial())
    }

    pub(crate) fn add_raw(self, rhs: Self) -> Result<Self, CoefficientError> {
        Ok(match (self, rhs) {
            (Function::Zero, rhs) => rhs,
            (lhs, Function::Zero) => lhs,
            (Function::Constant(lhs), Function::Constant(rhs)) => {
                if let Some(coefficient) = (lhs + rhs)? {
                    Function::Constant(coefficient)
                } else {
                    Function::Zero
                }
            }
            (Function::Constant(c), Function::Linear(mut l))
            | (Function::Linear(mut l), Function::Constant(c)) => {
                l.add_term(LinearMonomial::Constant, c)?;
                Function::Linear(l)
            }
            (Function::Constant(c), Function::Quadratic(mut q))
            | (Function::Quadratic(mut q), Function::Constant(c)) => {
                q.add_term(QuadraticMonomial::Constant, c)?;
                Function::Quadratic(q)
            }
            (Function::Constant(c), Function::Polynomial(mut p))
            | (Function::Polynomial(mut p), Function::Constant(c)) => {
                p.add_term(MonomialDyn::default(), c)?;
                Function::Polynomial(p)
            }
            (Function::Linear(lhs), Function::Linear(rhs)) => Function::Linear((lhs + rhs)?),
            (Function::Linear(l), Function::Quadratic(q))
            | (Function::Quadratic(q), Function::Linear(l)) => Function::Quadratic((q + &l)?),
            (Function::Linear(l), Function::Polynomial(p))
            | (Function::Polynomial(p), Function::Linear(l)) => Function::Polynomial((p + &l)?),
            (Function::Quadratic(lhs), Function::Quadratic(rhs)) => {
                Function::Quadratic((lhs + rhs)?)
            }
            (Function::Quadratic(q), Function::Polynomial(p))
            | (Function::Polynomial(p), Function::Quadratic(q)) => Function::Polynomial((p + &q)?),
            (Function::Polynomial(lhs), Function::Polynomial(rhs)) => {
                Function::Polynomial((lhs + rhs)?)
            }
        })
    }
}

impl Add for Function {
    type Output = Result<Self, CoefficientError>;

    fn add(self, rhs: Self) -> Self::Output {
        Ok(self.add_raw(rhs)?.normalize())
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
        Ok(Function::from_polynomial((self.into_polynomial() + rhs)?))
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
                Ok(Function::from_polynomial(
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
    use crate::{coeff, linear};
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

    #[test]
    fn arithmetic_normalizes_low_degree_results() {
        let linear = Function::from(linear!(1));
        let constant = Function::from(coeff!(2.0));

        assert!(matches!(
            (linear.clone() + constant).unwrap(),
            Function::Linear(_)
        ));
        assert!(matches!(
            (linear.clone() * Function::from(linear!(2))).unwrap(),
            Function::Quadratic(_)
        ));
        assert!(matches!(
            (linear.clone() / coeff!(2.0)).unwrap(),
            Function::Linear(_)
        ));
        assert!(matches!((linear.clone() - linear).unwrap(), Function::Zero));
    }
}
