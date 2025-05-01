use super::*;
use std::ops::{Sub, SubAssign};

impl SubAssign<&Function> for Function {
    fn sub_assign(&mut self, rhs: &Function) {
        match rhs {
            Function::Zero => {}
            Function::Constant(c) => self.sub_assign(*c),
            Function::Linear(l) => self.sub_assign(l),
            Function::Quadratic(q) => self.sub_assign(q),
            Function::Polynomial(p) => self.sub_assign(p),
        }
    }
}

impl SubAssign for Function {
    fn sub_assign(&mut self, rhs: Self) {
        match rhs {
            Function::Zero => {}
            Function::Constant(c) => self.sub_assign(c),
            Function::Linear(l) => self.sub_assign(l),
            Function::Quadratic(q) => self.sub_assign(q),
            Function::Polynomial(p) => self.sub_assign(p),
        }
    }
}

impl SubAssign<Coefficient> for Function {
    fn sub_assign(&mut self, rhs: Coefficient) {
        match self {
            Function::Zero => *self = Function::from(-rhs),
            Function::Constant(c) => {
                *self = (*c - rhs).map(Function::from).unwrap_or(Function::Zero)
            }
            Function::Linear(l) => l.sub_assign(rhs),
            Function::Quadratic(q) => q.sub_assign(rhs),
            Function::Polynomial(p) => p.sub_assign(rhs),
        }
    }
}

impl SubAssign<&Linear> for Function {
    fn sub_assign(&mut self, rhs: &Linear) {
        match self {
            Function::Linear(l) => l.sub_assign(rhs),
            Function::Quadratic(q) => q.sub_assign(rhs),
            Function::Polynomial(p) => p.sub_assign(rhs),
            _ => self.sub_assign(rhs.clone()),
        }
    }
}

impl SubAssign<Linear> for Function {
    fn sub_assign(&mut self, mut rhs: Linear) {
        match self {
            Function::Zero => *self = Function::from(-rhs),
            Function::Constant(c) => {
                rhs = -rhs;
                rhs += *c;
                *self = Function::from(rhs);
            }
            Function::Linear(l) => l.sub_assign(rhs),
            Function::Quadratic(q) => q.sub_assign(&rhs),
            Function::Polynomial(p) => p.sub_assign(&rhs),
        }
    }
}

impl SubAssign<&Quadratic> for Function {
    fn sub_assign(&mut self, rhs: &Quadratic) {
        match self {
            Function::Quadratic(q) => q.sub_assign(rhs),
            Function::Polynomial(p) => p.sub_assign(rhs),
            _ => self.sub_assign(rhs.clone()),
        }
    }
}

impl SubAssign<Quadratic> for Function {
    fn sub_assign(&mut self, mut rhs: Quadratic) {
        match self {
            Function::Zero => *self = Function::from(-rhs),
            Function::Constant(c) => {
                rhs = -rhs;
                rhs += *c;
                *self = Function::from(rhs);
            }
            Function::Linear(l) => {
                let mut q = -rhs;
                q += &*l;
                *self = Function::from(q);
            }
            Function::Quadratic(q1) => q1.sub_assign(rhs),
            Function::Polynomial(p) => p.sub_assign(&rhs),
        }
    }
}

impl SubAssign<&Polynomial> for Function {
    fn sub_assign(&mut self, rhs: &Polynomial) {
        match self {
            Function::Polynomial(p) => p.sub_assign(rhs),
            _ => self.sub_assign(rhs.clone()),
        }
    }
}

impl SubAssign<Polynomial> for Function {
    fn sub_assign(&mut self, mut rhs: Polynomial) {
        match self {
            Function::Zero => *self = Function::from(-rhs),
            Function::Constant(c) => {
                rhs = -rhs;
                rhs += *c;
                *self = Function::from(rhs);
            }
            Function::Linear(l) => {
                rhs = -rhs;
                rhs += &*l;
                *self = Function::from(rhs);
            }
            Function::Quadratic(q) => {
                rhs = -rhs;
                rhs += &*q;
                *self = Function::from(rhs);
            }
            Function::Polynomial(p) => p.sub_assign(rhs),
        }
    }
}

macro_rules! impl_sub_via_sub_assign {
    ($RHS:ty) => {
        impl Sub<$RHS> for Function {
            type Output = Self;
            fn sub(mut self, rhs: $RHS) -> Self::Output {
                self.sub_assign(rhs);
                self
            }
        }
    };
}

impl_sub_via_sub_assign!(Coefficient);
impl_sub_via_sub_assign!(Linear);
impl_sub_via_sub_assign!(&Linear);
impl_sub_via_sub_assign!(Quadratic);
impl_sub_via_sub_assign!(&Quadratic);
impl_sub_via_sub_assign!(Polynomial);
impl_sub_via_sub_assign!(&Polynomial);
impl_sub_via_sub_assign!(&Function);

impl Sub for Function {
    type Output = Self;
    fn sub(mut self, rhs: Self) -> Self::Output {
        self.sub_assign(rhs);
        self
    }
}

impl Sub for &Function {
    type Output = Function;
    fn sub(self, rhs: Self) -> Self::Output {
        if self.degree() < rhs.degree() {
            -rhs.clone() + self.clone()
        } else {
            self.clone() - rhs.clone()
        }
    }
}
