use super::*;
use num::Zero;
use std::{
    iter::Sum,
    ops::{Add, AddAssign, Neg, Sub, SubAssign},
};

impl<M: Monomial> AddAssign<&Polynomial<M>> for Polynomial<M> {
    fn add_assign(&mut self, rhs: &Self) {
        for (id, c) in &rhs.terms {
            self.add_term(id.clone(), *c)
        }
    }
}

impl<M: Monomial> AddAssign for Polynomial<M> {
    fn add_assign(&mut self, mut rhs: Self) {
        if self.terms.len() < rhs.terms.len() {
            rhs += &*self;
            *self = rhs;
        } else {
            self.add_assign(&rhs);
        }
    }
}

impl<M: Monomial> Add for Polynomial<M> {
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

impl<M: Monomial> Add<&Polynomial<M>> for Polynomial<M> {
    type Output = Self;
    fn add(mut self, rhs: &Self) -> Self::Output {
        self += rhs;
        self
    }
}

impl<M: Monomial> Add for &Polynomial<M> {
    type Output = Polynomial<M>;
    fn add(self, rhs: Self) -> Self::Output {
        if self.terms.len() < rhs.terms.len() {
            rhs.clone() + self
        } else {
            self.clone() + rhs
        }
    }
}

impl<M: Monomial> Add<Polynomial<M>> for &Polynomial<M> {
    type Output = Polynomial<M>;
    fn add(self, rhs: Polynomial<M>) -> Self::Output {
        rhs + self
    }
}

impl<M: Monomial> Sum for Polynomial<M> {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::default(), Add::add)
    }
}

impl<M: Monomial> Neg for Polynomial<M> {
    type Output = Self;
    fn neg(mut self) -> Self::Output {
        for c in self.terms.values_mut() {
            *c = -(*c);
        }
        self
    }
}

impl<M: Monomial> SubAssign<&Polynomial<M>> for Polynomial<M> {
    fn sub_assign(&mut self, rhs: &Polynomial<M>) {
        for (id, c) in &rhs.terms {
            self.add_term(id.clone(), -(*c));
        }
    }
}

impl<M: Monomial> SubAssign for Polynomial<M> {
    fn sub_assign(&mut self, rhs: Self) {
        if self.terms.len() < rhs.terms.len() {
            *self = -rhs + &*self;
        } else {
            self.sub_assign(&rhs);
        }
    }
}

impl<M: Monomial> Sub for Polynomial<M> {
    type Output = Self;
    fn sub(mut self, rhs: Self) -> Self::Output {
        self -= rhs;
        self
    }
}

impl<M: Monomial> Sub<&Polynomial<M>> for Polynomial<M> {
    type Output = Self;
    fn sub(mut self, rhs: &Self) -> Self::Output {
        self -= rhs;
        self
    }
}

impl<M: Monomial> Sub for &Polynomial<M> {
    type Output = Polynomial<M>;
    fn sub(self, rhs: Self) -> Self::Output {
        if self.terms.len() < rhs.terms.len() {
            -rhs.clone() + self
        } else {
            self.clone() - rhs
        }
    }
}

impl<M: Monomial> Sub<Polynomial<M>> for &Polynomial<M> {
    type Output = Polynomial<M>;
    fn sub(self, rhs: Polynomial<M>) -> Self::Output {
        -rhs + self
    }
}

impl<M: Monomial> Zero for Polynomial<M> {
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
    use ::approx::assert_abs_diff_eq;
    use proptest::prelude::*;

    type Linear = Polynomial<LinearMonomial>;

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
            assert_abs_diff_eq!(&a + (&b + &c), (&a + &b) + &c, epsilon = 1e-9);
        }
    }
}
