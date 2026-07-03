use super::*;
use crate::{CoefficientError, LinearMonomial, Monomial, MonomialDyn, QuadraticMonomial};
use std::ops::{Add, Neg};

impl Function {
    pub fn zero() -> Self {
        Function::Zero
    }

    pub fn is_zero(&self) -> bool {
        matches!(self, Function::Zero)
    }

    /// Re-canonicalize the variant after an arithmetic operation.
    ///
    /// Arithmetic can lower the actual degree (e.g. all quadratic terms
    /// cancel), so the result may need to be downgraded to a lower-degree
    /// variant. This only inspects the monomials already stored in the term
    /// map and rebuilds it exclusively when a downgrade actually happens; the
    /// common case (variant already canonical) is a key scan that stops at
    /// the first monomial of maximal degree.
    pub(crate) fn normalize(self) -> Self {
        fn constant_term<M: crate::Monomial>(p: &crate::PolynomialBase<M>) -> Coefficient {
            p.get(&M::default())
                .expect("non-zero degree-0 polynomial has a constant term")
        }
        match self {
            Function::Zero | Function::Constant(_) => self,
            Function::Linear(l) => {
                if l.is_zero() {
                    Function::Zero
                } else if l.degree() == 0 {
                    Function::Constant(constant_term(&l))
                } else {
                    Function::Linear(l)
                }
            }
            Function::Quadratic(q) => {
                if q.is_zero() {
                    return Function::Zero;
                }
                match q.degree().into_inner() {
                    2 => Function::Quadratic(q),
                    1 => Function::Linear(
                        Linear::try_from(&q).expect("degree-1 polynomial is linear"),
                    ),
                    _ => Function::Constant(constant_term(&q)),
                }
            }
            Function::Polynomial(p) => {
                if p.is_zero() {
                    return Function::Zero;
                }
                let mut degree = Degree::from(0);
                for monomial in p.keys() {
                    degree = degree.max(monomial.degree());
                    if degree > 2 {
                        // The variant cannot be downgraded; stop scanning.
                        break;
                    }
                }
                if degree > 2 {
                    return Function::Polynomial(p);
                }
                match degree.into_inner() {
                    2 => Function::Quadratic(
                        Quadratic::try_from(&p).expect("degree-2 polynomial is quadratic"),
                    ),
                    1 => Function::Linear(
                        Linear::try_from(&p).expect("degree-1 polynomial is linear"),
                    ),
                    _ => Function::Constant(constant_term(&p)),
                }
            }
        }
    }

    /// Add `rhs` to this function in place.
    ///
    /// This is a fallible replacement for `AddAssign`: it returns
    /// [`CoefficientError`] when coefficient arithmetic overflows or produces
    /// NaN. The operation is intentionally not atomic. If an error is returned,
    /// `self` may already have been modified.
    pub fn try_add_assign_in_place(&mut self, rhs: Self) -> Result<(), CoefficientError> {
        let lhs = std::mem::take(self);
        *self = match (lhs, rhs) {
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
        };
        Ok(())
    }
}

impl Add for Function {
    type Output = Result<Self, CoefficientError>;

    fn add(self, rhs: Self) -> Self::Output {
        let mut out = self;
        out.try_add_assign_in_place(rhs)?;
        Ok(out.normalize())
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

    fn add(mut self, rhs: Coefficient) -> Self::Output {
        self.try_add_assign_in_place(Function::Constant(rhs))?;
        Ok(self.normalize())
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
    (& $rhs:ty) => {
        impl Add<&$rhs> for Function {
            type Output = Result<Function, CoefficientError>;

            fn add(mut self, rhs: &$rhs) -> Self::Output {
                self.try_add_assign_in_place(Function::from(rhs.clone()))?;
                Ok(self.normalize())
            }
        }

        impl Add<&$rhs> for &Function {
            type Output = Result<Function, CoefficientError>;

            fn add(self, rhs: &$rhs) -> Self::Output {
                self.clone() + rhs
            }
        }
    };
    ($rhs:ty) => {
        impl Add<$rhs> for Function {
            type Output = Result<Function, CoefficientError>;

            fn add(mut self, rhs: $rhs) -> Self::Output {
                self.try_add_assign_in_place(Function::from(rhs))?;
                Ok(self.normalize())
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
