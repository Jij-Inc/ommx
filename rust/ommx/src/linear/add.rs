use super::Linear;
use num::Zero;
use std::{
    iter::Sum,
    ops::{Add, AddAssign},
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

impl Zero for Linear {
    fn zero() -> Self {
        Self::default()
    }
    fn is_zero(&self) -> bool {
        self.terms.is_empty() && self.constant.is_zero()
    }
}
