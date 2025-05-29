use super::*;
use crate::{LinearMonomial, MonomialDyn, QuadraticMonomial};
use num::Zero;
use std::{
    iter::Sum,
    ops::{Add, AddAssign, Neg, Sub, SubAssign},
};

impl<M1, M2> AddAssign<&PolynomialBase<M1>> for PolynomialBase<M2>
where
    M1: Monomial,
    M2: Monomial + From<M1>,
{
    fn add_assign(&mut self, rhs: &PolynomialBase<M1>) {
        for (id, c) in &rhs.terms {
            self.add_term(id.clone().into(), *c)
        }
    }
}

impl<M: Monomial> AddAssign for PolynomialBase<M> {
    fn add_assign(&mut self, mut rhs: Self) {
        if self.terms.len() < rhs.terms.len() {
            rhs += &*self;
            *self = rhs;
        } else {
            self.add_assign(&rhs);
        }
    }
}

impl<M: Monomial> AddAssign<Coefficient> for PolynomialBase<M> {
    fn add_assign(&mut self, rhs: Coefficient) {
        self.add_term(M::default(), rhs);
    }
}

impl<M: Monomial> Add<Coefficient> for PolynomialBase<M> {
    type Output = Self;
    fn add(mut self, rhs: Coefficient) -> Self::Output {
        self += rhs;
        self
    }
}

impl<M: Monomial> Add<PolynomialBase<M>> for Coefficient {
    type Output = PolynomialBase<M>;
    fn add(self, mut rhs: PolynomialBase<M>) -> Self::Output {
        rhs += self;
        rhs
    }
}

impl<M: Monomial> Add<Coefficient> for &PolynomialBase<M> {
    type Output = PolynomialBase<M>;
    fn add(self, rhs: Coefficient) -> Self::Output {
        self.clone() + rhs
    }
}

impl<M: Monomial> Add<&PolynomialBase<M>> for Coefficient {
    type Output = PolynomialBase<M>;
    fn add(self, rhs: &PolynomialBase<M>) -> Self::Output {
        self + rhs.clone()
    }
}

// Add support for Monomial + Monomial operations for specific monomial types
macro_rules! impl_monomial_add {
    ($monomial:ty) => {
        impl Add for $monomial {
            type Output = PolynomialBase<$monomial>;
            fn add(self, rhs: Self) -> Self::Output {
                PolynomialBase::from(self) + PolynomialBase::from(rhs)
            }
        }

        impl Add<&$monomial> for $monomial {
            type Output = PolynomialBase<$monomial>;
            fn add(self, rhs: &Self) -> Self::Output {
                PolynomialBase::from(self) + PolynomialBase::from(rhs.clone())
            }
        }

        impl Add<$monomial> for &$monomial {
            type Output = PolynomialBase<$monomial>;
            fn add(self, rhs: $monomial) -> Self::Output {
                PolynomialBase::from(self.clone()) + PolynomialBase::from(rhs)
            }
        }

        impl Add for &$monomial {
            type Output = PolynomialBase<$monomial>;
            fn add(self, rhs: Self) -> Self::Output {
                PolynomialBase::from(self.clone()) + PolynomialBase::from(rhs.clone())
            }
        }

        // Add support for Monomial + Coefficient operations
        impl Add<Coefficient> for $monomial {
            type Output = PolynomialBase<$monomial>;
            fn add(self, rhs: Coefficient) -> Self::Output {
                PolynomialBase::from(self) + rhs
            }
        }

        impl Add<$monomial> for Coefficient {
            type Output = PolynomialBase<$monomial>;
            fn add(self, rhs: $monomial) -> Self::Output {
                self + PolynomialBase::from(rhs)
            }
        }

        impl Add<Coefficient> for &$monomial {
            type Output = PolynomialBase<$monomial>;
            fn add(self, rhs: Coefficient) -> Self::Output {
                PolynomialBase::from(self.clone()) + rhs
            }
        }

        impl Add<&$monomial> for Coefficient {
            type Output = PolynomialBase<$monomial>;
            fn add(self, rhs: &$monomial) -> Self::Output {
                self + PolynomialBase::from(rhs.clone())
            }
        }
    };
}

impl_monomial_add!(LinearMonomial);
impl_monomial_add!(QuadraticMonomial);
impl_monomial_add!(MonomialDyn);

impl<M: Monomial> Add for PolynomialBase<M> {
    type Output = Self;
    fn add(mut self, mut rhs: Self) -> Self::Output {
        if self.terms.len() < rhs.terms.len() {
            rhs += self;
            rhs
        } else {
            self += rhs;
            self
        }
    }
}

impl<M1: Monomial, M2: Monomial + From<M1>> Add<&PolynomialBase<M1>> for PolynomialBase<M2> {
    type Output = Self;
    fn add(mut self, rhs: &PolynomialBase<M1>) -> Self::Output {
        self += rhs;
        self
    }
}

impl<M: Monomial> Add for &PolynomialBase<M> {
    type Output = PolynomialBase<M>;
    fn add(self, rhs: Self) -> Self::Output {
        if self.terms.len() < rhs.terms.len() {
            rhs.clone() + self
        } else {
            self.clone() + rhs
        }
    }
}

impl<M: Monomial> Add<PolynomialBase<M>> for &PolynomialBase<M> {
    type Output = PolynomialBase<M>;
    fn add(self, rhs: PolynomialBase<M>) -> Self::Output {
        rhs + self
    }
}

impl<M: Monomial> Sum for PolynomialBase<M> {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::default(), Add::add)
    }
}

impl<M: Monomial> Neg for PolynomialBase<M> {
    type Output = Self;
    fn neg(mut self) -> Self::Output {
        for c in self.terms.values_mut() {
            *c = -(*c);
        }
        self
    }
}

impl<M1: Monomial, M2: Monomial + From<M1>> SubAssign<&PolynomialBase<M1>> for PolynomialBase<M2> {
    fn sub_assign(&mut self, rhs: &PolynomialBase<M1>) {
        for (id, c) in &rhs.terms {
            self.add_term(id.clone().into(), -(*c));
        }
    }
}

impl<M: Monomial> SubAssign for PolynomialBase<M> {
    fn sub_assign(&mut self, rhs: Self) {
        if self.terms.len() < rhs.terms.len() {
            *self = -rhs + &*self;
        } else {
            self.sub_assign(&rhs);
        }
    }
}

impl<M: Monomial> SubAssign<Coefficient> for PolynomialBase<M> {
    fn sub_assign(&mut self, rhs: Coefficient) {
        self.add_term(M::default(), -rhs);
    }
}

impl<M: Monomial> Sub for PolynomialBase<M> {
    type Output = Self;
    fn sub(mut self, rhs: Self) -> Self::Output {
        self -= rhs;
        self
    }
}

impl<M1: Monomial, M2: Monomial + From<M1>> Sub<&PolynomialBase<M1>> for PolynomialBase<M2> {
    type Output = Self;
    fn sub(mut self, rhs: &PolynomialBase<M1>) -> Self::Output {
        self -= rhs;
        self
    }
}

impl<M: Monomial> Sub for &PolynomialBase<M> {
    type Output = PolynomialBase<M>;
    fn sub(self, rhs: Self) -> Self::Output {
        if self.terms.len() < rhs.terms.len() {
            -rhs.clone() + self
        } else {
            self.clone() - rhs
        }
    }
}

impl<M: Monomial> Sub<PolynomialBase<M>> for &PolynomialBase<M> {
    type Output = PolynomialBase<M>;
    fn sub(self, rhs: PolynomialBase<M>) -> Self::Output {
        -rhs + self
    }
}

impl<M: Monomial> Zero for PolynomialBase<M> {
    fn zero() -> Self {
        Self::default()
    }
    fn is_zero(&self) -> bool {
        self.terms.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VariableID;
    use ::approx::assert_abs_diff_eq;
    use proptest::prelude::*;

    type Linear = PolynomialBase<LinearMonomial>;

    #[test]
    fn test_monomial_add() {
        // Test LinearMonomial + LinearMonomial
        let x1 = LinearMonomial::Variable(VariableID::from(1));
        let x2 = LinearMonomial::Variable(VariableID::from(2));
        let result = x1 + x2;

        // Simple addition of monomials
        let expected = x1 + x2;
        assert_abs_diff_eq!(result, expected);

        // Test QuadraticMonomial + QuadraticMonomial
        let q1 = QuadraticMonomial::Linear(VariableID::from(1));
        let q2 = QuadraticMonomial::Linear(VariableID::from(2));
        let result = q1 + q2;

        // Simple addition of monomials
        let expected = q1 + q2;
        assert_abs_diff_eq!(result, expected);
    }

    #[test]
    fn test_monomial_coefficient_add() {
        // Test Monomial + Coefficient
        let x1 = LinearMonomial::Variable(VariableID::from(1));
        let coef = Coefficient::try_from(2.0).unwrap();
        let result = x1 + coef;
        let expected = PolynomialBase::from(x1) + coef;
        assert_abs_diff_eq!(result, expected);

        // Test Coefficient + Monomial
        let x2 = LinearMonomial::Variable(VariableID::from(2));
        let coef = Coefficient::try_from(3.0).unwrap();
        let result = coef + x2;
        let expected = coef + PolynomialBase::from(x2);
        assert_abs_diff_eq!(result, expected);

        // Test &Monomial + Coefficient
        let x3 = LinearMonomial::Variable(VariableID::from(3));
        let coef = Coefficient::try_from(4.0).unwrap();
        let result = &x3 + coef;
        let expected = PolynomialBase::from(x3.clone()) + coef;
        assert_abs_diff_eq!(result, expected);

        // Test Coefficient + &Monomial
        let x4 = LinearMonomial::Variable(VariableID::from(4));
        let coef = Coefficient::try_from(5.0).unwrap();
        let result = coef + &x4;
        let expected = coef + PolynomialBase::from(x4.clone());
        assert_abs_diff_eq!(result, expected);
    }

    proptest! {
        /// Check four implementations of Add yields the same result
        #[test]
        fn add_ref(a: Linear, b: Linear) {
            let ans = a.clone() + b.clone();
            assert_abs_diff_eq!(&a + &b, ans);
            assert_abs_diff_eq!(&a + b.clone(), ans);
            assert_abs_diff_eq!(a + &b, ans);
        }

        /// Check four implementations of Sub yields the same result
        #[test]
        fn sub_ref(a: Linear, b: Linear) {
            let ans = a.clone() - b.clone();
            assert_abs_diff_eq!(&a - &b, ans);
            assert_abs_diff_eq!(&a - b.clone(), ans);
            assert_abs_diff_eq!(a - &b, ans);
        }

        #[test]
        fn zero(a: Linear) {
            assert_abs_diff_eq!(&a + Linear::zero(), &a);
            assert_abs_diff_eq!(&a - Linear::zero(), &a);
            assert_abs_diff_eq!(&a - &a, Linear::zero());
        }

        #[test]
        fn add_commutative(a: Linear, b: Linear) {
            assert_abs_diff_eq!(&a + &b, &b + &a);
        }

        #[test]
        fn add_associative(a: Linear, b: Linear, c: Linear) {
            assert_abs_diff_eq!(&a + (&b + &c), (&a + &b) + &c);
        }
    }
}
