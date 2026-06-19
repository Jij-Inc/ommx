use std::ops::{Div, DivAssign};

use super::*;

impl DivAssign<Coefficient> for Function {
    fn div_assign(&mut self, rhs: Coefficient) {
        match self {
            Function::Zero => {}
            Function::Constant(c) => *c /= rhs,
            Function::Linear(l) => l.values_mut().for_each(|coefficient| *coefficient /= rhs),
            Function::Quadratic(q) => q.values_mut().for_each(|coefficient| *coefficient /= rhs),
            Function::Polynomial(p) => p.values_mut().for_each(|coefficient| *coefficient /= rhs),
        }
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
    use crate::{coeff, linear};
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

    #[test]
    fn div_by_zero_coefficient_created_by_arithmetic_produces_infinities() {
        let tiny = Coefficient::try_from(f64::from_bits(1)).unwrap();
        let zero = tiny * tiny;
        assert_eq!(zero.into_inner(), 0.0);

        let function =
            Function::from(coeff!(2.0) * linear!(1) + (-coeff!(3.0)) * linear!(2) + coeff!(4.0));
        let divided = function / zero;
        let values = divided
            .values()
            .map(|coefficient| coefficient.into_inner())
            .collect::<Vec<_>>();

        assert_eq!(values.len(), 3);
        assert!(values.iter().all(|value| value.is_infinite()));
        assert!(values.iter().any(|value| value.is_sign_positive()));
        assert!(values.iter().any(|value| value.is_sign_negative()));
    }

    #[test]
    fn zero_function_div_by_zero_coefficient_is_noop() {
        let tiny = Coefficient::try_from(f64::from_bits(1)).unwrap();
        let zero = tiny * tiny;
        assert_abs_diff_eq!(Function::zero() / zero, Function::zero());
    }

    #[test]
    #[should_panic]
    fn zero_coefficient_div_by_zero_coefficient_panics() {
        let tiny = Coefficient::try_from(f64::from_bits(1)).unwrap();
        let zero = tiny * tiny;
        let _ = Function::from(zero) / zero;
    }
}
