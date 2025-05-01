use std::ops::{Mul, MulAssign};

use super::*;

impl MulAssign<Coefficient> for Function {
    fn mul_assign(&mut self, rhs: Coefficient) {
        match self {
            Function::Zero => {}
            Function::Constant(c) => *c *= rhs,
            Function::Linear(l) => l.mul_assign(rhs),
            Function::Quadratic(q) => q.mul_assign(rhs),
            Function::Polynomial(p) => p.mul_assign(rhs),
        }
    }
}

impl MulAssign<&Linear> for Function {
    fn mul_assign(&mut self, rhs: &Linear) {
        match self {
            Function::Zero => {}
            Function::Constant(c) => *self = (rhs.clone() * *c).into(),
            Function::Linear(l) => *self = (&*l * rhs).into(),
            Function::Quadratic(q) => *self = (&*q * rhs).into(),
            Function::Polynomial(p) => *self = (&*p * rhs).into(),
        }
    }
}

impl MulAssign<Linear> for Function {
    fn mul_assign(&mut self, rhs: Linear) {
        match self {
            Function::Constant(c) => *self = (rhs * *c).into(),
            _ => self.mul_assign(&rhs),
        }
    }
}

impl MulAssign<&Quadratic> for Function {
    fn mul_assign(&mut self, rhs: &Quadratic) {
        match self {
            Function::Zero => {}
            Function::Constant(c) => *self = (rhs.clone() * *c).into(),
            Function::Linear(l) => *self = (&*l * rhs).into(),
            Function::Quadratic(q) => *self = (&*q * rhs).into(),
            Function::Polynomial(p) => *self = (&*p * rhs).into(),
        }
    }
}

impl MulAssign<Quadratic> for Function {
    fn mul_assign(&mut self, rhs: Quadratic) {
        match self {
            Function::Constant(c) => *self = (rhs * *c).into(),
            _ => self.mul_assign(&rhs),
        }
    }
}

impl MulAssign<&Polynomial> for Function {
    fn mul_assign(&mut self, rhs: &Polynomial) {
        match self {
            Function::Zero => {}
            Function::Constant(c) => *self = (rhs.clone() * *c).into(),
            Function::Linear(l) => *self = (&*l * rhs).into(),
            Function::Quadratic(q) => *self = (&*q * rhs).into(),
            Function::Polynomial(p) => *self = (&*p * rhs).into(),
        }
    }
}

impl MulAssign<Polynomial> for Function {
    fn mul_assign(&mut self, rhs: Polynomial) {
        match self {
            Function::Constant(c) => *self = (rhs * *c).into(),
            _ => self.mul_assign(&rhs),
        }
    }
}

macro_rules! impl_mul_via_mul_assign {
    ($rhs:ty) => {
        impl Mul<$rhs> for Function {
            type Output = Self;
            fn mul(mut self, rhs: $rhs) -> Self {
                self.mul_assign(rhs);
                self
            }
        }
    };
    () => {};
}

impl_mul_via_mul_assign!(Coefficient);
impl_mul_via_mul_assign!(&Linear);
impl_mul_via_mul_assign!(Linear);
impl_mul_via_mul_assign!(&Quadratic);
impl_mul_via_mul_assign!(Quadratic);
impl_mul_via_mul_assign!(&Polynomial);
impl_mul_via_mul_assign!(Polynomial);

impl MulAssign for Function {
    fn mul_assign(&mut self, rhs: Self) {
        match rhs {
            Function::Zero => *self = Function::Zero,
            Function::Constant(c) => self.mul_assign(c),
            Function::Linear(l) => self.mul_assign(l),
            Function::Quadratic(q) => self.mul_assign(q),
            Function::Polynomial(p) => self.mul_assign(p),
        }
    }
}

impl MulAssign<&Function> for Function {
    fn mul_assign(&mut self, rhs: &Self) {
        match rhs {
            Function::Zero => *self = Function::Zero,
            Function::Constant(c) => self.mul_assign(*c),
            Function::Linear(l) => self.mul_assign(l),
            Function::Quadratic(q) => self.mul_assign(q),
            Function::Polynomial(p) => self.mul_assign(p),
        }
    }
}
