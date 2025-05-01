use std::ops::AddAssign;

use super::*;

impl AddAssign<&Function> for Function {
    fn add_assign(&mut self, rhs: &Function) {
        match rhs {
            Function::Zero => {}
            Function::Constant(c) => self.add_assign(*c),
            Function::Linear(l) => self.add_assign(l),
            Function::Quadratic(q) => self.add_assign(q),
            Function::Polynomial(p) => self.add_assign(p),
        }
    }
}

impl AddAssign for Function {
    fn add_assign(&mut self, rhs: Self) {
        match rhs {
            Function::Zero => {}
            Function::Constant(c) => self.add_assign(c),
            Function::Linear(l) => self.add_assign(l),
            Function::Quadratic(q) => self.add_assign(q),
            Function::Polynomial(p) => self.add_assign(p),
        }
    }
}

impl AddAssign<Coefficient> for Function {
    fn add_assign(&mut self, rhs: Coefficient) {
        match self {
            Function::Zero => *self = Function::from(rhs),
            Function::Constant(c) => {
                if let Some(v) = *c + rhs {
                    *self = Function::from(v);
                } else {
                    *self = Function::Zero;
                }
            }
            Function::Linear(l) => l.add_assign(rhs),
            Function::Quadratic(q) => q.add_assign(rhs),
            Function::Polynomial(p) => p.add_assign(rhs),
        }
    }
}

impl AddAssign<&Linear> for Function {
    fn add_assign(&mut self, rhs: &Linear) {
        match self {
            Function::Linear(l) => l.add_assign(rhs),
            Function::Quadratic(q) => q.add_assign(rhs),
            Function::Polynomial(p) => p.add_assign(rhs),
            _ => self.add_assign(rhs.clone()),
        }
    }
}

impl AddAssign<Linear> for Function {
    fn add_assign(&mut self, mut rhs: Linear) {
        match self {
            Function::Zero => *self = Function::from(rhs),
            Function::Constant(c) => {
                rhs += *c;
                *self = Function::from(rhs);
            }
            Function::Linear(l) => l.add_assign(rhs),
            Function::Quadratic(q) => q.add_assign(&rhs),
            Function::Polynomial(p) => p.add_assign(&rhs),
        }
    }
}

impl AddAssign<&Quadratic> for Function {
    fn add_assign(&mut self, rhs: &Quadratic) {
        match self {
            Function::Quadratic(q) => q.add_assign(rhs),
            Function::Polynomial(p) => p.add_assign(rhs),
            _ => self.add_assign(rhs.clone()),
        }
    }
}

impl AddAssign<Quadratic> for Function {
    fn add_assign(&mut self, mut rhs: Quadratic) {
        match self {
            Function::Zero => *self = Function::from(rhs),
            Function::Constant(c) => {
                rhs += *c;
                *self = Function::from(rhs);
            }
            Function::Linear(l) => {
                rhs += &*l;
                *self = Function::from(rhs);
            }
            Function::Quadratic(q) => q.add_assign(rhs),
            Function::Polynomial(p) => p.add_assign(&rhs),
        }
    }
}

impl AddAssign<&Polynomial> for Function {
    fn add_assign(&mut self, rhs: &Polynomial) {
        match self {
            Function::Polynomial(p) => p.add_assign(rhs),
            _ => self.add_assign(rhs.clone()),
        }
    }
}

impl AddAssign<Polynomial> for Function {
    fn add_assign(&mut self, mut rhs: Polynomial) {
        match self {
            Function::Zero => *self = Function::from(rhs),
            Function::Constant(c) => {
                rhs += *c;
                *self = Function::from(rhs);
            }
            Function::Linear(l) => {
                rhs += &*l;
                *self = Function::from(rhs);
            }
            Function::Quadratic(q) => {
                rhs += &*q;
                *self = Function::from(rhs);
            }
            Function::Polynomial(p) => p.add_assign(rhs),
        }
    }
}
