use super::*;
use num::Zero;
use std::ops::{Add, AddAssign, Neg};

impl Zero for Function {
    fn zero() -> Self {
        Function::Zero
    }
    fn is_zero(&self) -> bool {
        matches!(self, Function::Zero)
    }
}

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
                *self = (*c + rhs).map(Function::from).unwrap_or(Function::Zero)
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

macro_rules! impl_add_via_add_assign {
    ($RHS:ty) => {
        impl Add<$RHS> for Function {
            type Output = Self;
            fn add(mut self, rhs: $RHS) -> Self::Output {
                self.add_assign(rhs);
                self
            }
        }
    };
}

impl_add_via_add_assign!(Coefficient);
impl_add_via_add_assign!(Linear);
impl_add_via_add_assign!(&Linear);
impl_add_via_add_assign!(Quadratic);
impl_add_via_add_assign!(&Quadratic);
impl_add_via_add_assign!(Polynomial);
impl_add_via_add_assign!(&Polynomial);
impl_add_via_add_assign!(&Function);

impl Add for Function {
    type Output = Self;
    fn add(mut self, rhs: Self) -> Self::Output {
        self += rhs;
        self
    }
}

impl Add for &Function {
    type Output = Function;
    fn add(self, rhs: Self) -> Self::Output {
        if self.degree() > rhs.degree() {
            self.clone() + rhs
        } else {
            rhs.clone() + self
        }
    }
}

impl Add<Function> for &Function {
    type Output = Function;
    fn add(self, rhs: Function) -> Self::Output {
        rhs + self
    }
}

impl Neg for Function {
    type Output = Self;
    fn neg(mut self) -> Self::Output {
        self.values_mut().for_each(|v| *v = -(*v));
        self
    }
}

// Add support for &Function operations with references
impl Add<&Coefficient> for &Function {
    type Output = Function;
    fn add(self, rhs: &Coefficient) -> Self::Output {
        self.clone() + *rhs
    }
}

impl Add<Coefficient> for &Function {
    type Output = Function;
    fn add(self, rhs: Coefficient) -> Self::Output {
        self.clone() + rhs
    }
}

impl Add<&Linear> for &Function {
    type Output = Function;
    fn add(self, rhs: &Linear) -> Self::Output {
        self.clone() + rhs
    }
}

impl Add<&Quadratic> for &Function {
    type Output = Function;
    fn add(self, rhs: &Quadratic) -> Self::Output {
        self.clone() + rhs
    }
}

impl Add<&Polynomial> for &Function {
    type Output = Function;
    fn add(self, rhs: &Polynomial) -> Self::Output {
        self.clone() + rhs
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
            let ans = a.clone() + b.clone();
            assert_abs_diff_eq!(&a + &b, ans);
            assert_abs_diff_eq!(&a + b.clone(), ans);
            assert_abs_diff_eq!(a + &b, ans);
        }

        #[test]
        fn zero(a in any::<Function>()) {
            assert_abs_diff_eq!(&a + Function::zero(), a.clone());
            assert_abs_diff_eq!(Function::zero() + &a, a.clone());
        }

        #[test]
        fn add_commutative(a in any::<Function>(), b in any::<Function>()) {
            assert_abs_diff_eq!(&a + &b, &b + &a);
        }

        #[test]
        fn add_associative(a in any::<Function>(), b in any::<Function>(), c in any::<Function>()) {
            assert_abs_diff_eq!(&a + (&b + &c), (&a + &b) + &c);
        }
    }
}
