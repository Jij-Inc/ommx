use std::ops::{Div, DivAssign};

use super::*;

impl DivAssign<Coefficient> for Function {
    fn div_assign(&mut self, rhs: Coefficient) {
        self.values_mut()
            .for_each(|coefficient| *coefficient /= rhs);
    }
}

impl DivAssign<&Coefficient> for Function {
    fn div_assign(&mut self, rhs: &Coefficient) {
        *self /= *rhs;
    }
}

impl Div<Coefficient> for Function {
    type Output = Self;

    fn div(mut self, rhs: Coefficient) -> Self::Output {
        self /= rhs;
        self
    }
}

impl Div<&Coefficient> for Function {
    type Output = Self;

    fn div(mut self, rhs: &Coefficient) -> Self::Output {
        self /= rhs;
        self
    }
}

impl Div<Coefficient> for &Function {
    type Output = Function;

    fn div(self, rhs: Coefficient) -> Self::Output {
        self.clone() / rhs
    }
}

impl Div<&Coefficient> for &Function {
    type Output = Function;

    fn div(self, rhs: &Coefficient) -> Self::Output {
        self.clone() / rhs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::approx::assert_abs_diff_eq;
    use num::{One, Zero};
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn div_ref(a in any::<Function>(), c in any::<Coefficient>()) {
            let expected = a.clone() / c;
            assert_abs_diff_eq!(&a / c, expected);
            assert_abs_diff_eq!(a.clone() / &c, expected);
            assert_abs_diff_eq!(&a / &c, expected);
        }

        #[test]
        fn div_inverse_of_mul(a in any::<Function>(), c in any::<Coefficient>()) {
            assert_abs_diff_eq!((a.clone() / c) * c, a);
        }

        #[test]
        fn zero(a in any::<Coefficient>()) {
            assert_abs_diff_eq!(Function::zero() / a, Function::zero());
        }
    }

    #[test]
    fn div_uses_direct_floating_point_division() {
        let tiny = Coefficient::try_from(f64::from_bits(1)).unwrap();
        assert_abs_diff_eq!(
            Function::from(tiny) / tiny,
            Function::from(Coefficient::one())
        );
    }
}
