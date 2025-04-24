use super::Linear;
use num::Zero;
use std::{
    iter::Sum,
    ops::{Add, AddAssign, Neg, Sub, SubAssign},
};

impl AddAssign<&Linear> for Linear {
    fn add_assign(&mut self, rhs: &Self) {
        for (id, c) in &rhs.terms {
            self.add_term(*id, *c)
        }
        self.constant += rhs.constant;
    }
}

impl AddAssign for Linear {
    fn add_assign(&mut self, mut rhs: Self) {
        if self.terms.len() < rhs.terms.len() {
            rhs += &*self;
            *self = rhs;
        } else {
            self.add_assign(&rhs);
        }
    }
}

impl Add for Linear {
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

impl Add<&Linear> for Linear {
    type Output = Self;
    fn add(mut self, rhs: &Self) -> Self::Output {
        self += rhs;
        self
    }
}

impl Add for &Linear {
    type Output = Linear;
    fn add(self, rhs: Self) -> Self::Output {
        if self.terms.len() < rhs.terms.len() {
            rhs.clone() + self
        } else {
            self.clone() + rhs
        }
    }
}

impl Add<Linear> for &Linear {
    type Output = Linear;
    fn add(self, rhs: Linear) -> Self::Output {
        rhs + self
    }
}

impl Sum for Linear {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Linear::default(), Add::add)
    }
}

impl Neg for Linear {
    type Output = Self;
    fn neg(mut self) -> Self::Output {
        for c in self.terms.values_mut() {
            *c = -(*c);
        }
        self.constant = -self.constant;
        self
    }
}

impl SubAssign<&Linear> for Linear {
    fn sub_assign(&mut self, rhs: &Linear) {
        for (id, c) in &rhs.terms {
            self.add_term(*id, -(*c));
        }
        self.constant -= rhs.constant;
    }
}

impl SubAssign for Linear {
    fn sub_assign(&mut self, rhs: Self) {
        if self.terms.len() < rhs.terms.len() {
            *self = -rhs + &*self;
        } else {
            self.sub_assign(&rhs);
        }
    }
}

impl Sub for Linear {
    type Output = Self;
    fn sub(mut self, rhs: Self) -> Self::Output {
        self -= rhs;
        self
    }
}

impl Sub<&Linear> for Linear {
    type Output = Self;
    fn sub(mut self, rhs: &Self) -> Self::Output {
        self -= rhs;
        self
    }
}

impl Sub for &Linear {
    type Output = Linear;
    fn sub(self, rhs: Self) -> Self::Output {
        if self.terms.len() < rhs.terms.len() {
            -rhs.clone() + self
        } else {
            self.clone() - rhs
        }
    }
}

impl Sub<Linear> for &Linear {
    type Output = Linear;
    fn sub(self, rhs: Linear) -> Self::Output {
        -rhs + self
    }
}

impl Zero for Linear {
    fn zero() -> Self {
        Self::default()
    }
    fn is_zero(&self) -> bool {
        self.terms.is_empty() && self.constant.is_zero()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;
    use proptest::prelude::*;

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
