//! Additional trait implementations for generated codes

macro_rules! impl_add_inverse {
    ($lhs:ty, $rhs:ty) => {
        impl ::std::ops::Add<$rhs> for $lhs {
            type Output = <$rhs as ::std::ops::Add<$lhs>>::Output;
            fn add(self, rhs: $rhs) -> Self::Output {
                rhs + self
            }
        }
    };
}
pub(crate) use impl_add_inverse;

macro_rules! impl_add_from {
    ($lhs:ty, $rhs:ty) => {
        impl ::std::ops::Add<$rhs> for $lhs {
            type Output = $lhs;
            fn add(self, rhs: $rhs) -> Self::Output {
                self + <$lhs>::from(rhs)
            }
        }
    };
}
pub(crate) use impl_add_from;

macro_rules! impl_sub_by_neg_add {
    ($lhs:ty, $rhs:ty) => {
        impl ::std::ops::Sub<$rhs> for $lhs {
            type Output = $lhs;
            fn sub(self, rhs: $rhs) -> Self::Output {
                self + (-rhs)
            }
        }
    };
}
pub(crate) use impl_sub_by_neg_add;

macro_rules! impl_mul_inverse {
    ($lhs:ty, $rhs:ty) => {
        impl ::std::ops::Mul<$rhs> for $lhs {
            type Output = <$rhs as ::std::ops::Mul<$lhs>>::Output;
            fn mul(self, rhs: $rhs) -> Self::Output {
                rhs * self
            }
        }
    };
}
pub(crate) use impl_mul_inverse;

macro_rules! impl_mul_from {
    ($lhs:ty, $rhs:ty, $output:ty) => {
        impl ::std::ops::Mul<$rhs> for $lhs {
            type Output = $output;
            fn mul(self, rhs: $rhs) -> Self::Output {
                self * <$lhs>::from(rhs)
            }
        }
    };
}
pub(crate) use impl_mul_from;

macro_rules! impl_neg_by_mul {
    ($ty:ty) => {
        impl ::std::ops::Neg for $ty {
            type Output = $ty;
            fn neg(self) -> Self::Output {
                self * -1.0
            }
        }

        impl ::std::ops::Neg for &$ty {
            type Output = $ty;
            fn neg(self) -> Self::Output {
                self.clone() * -1.0
            }
        }
    };
}
pub(crate) use impl_neg_by_mul;

#[cfg(test)]
macro_rules! test_algebraic {
    ($target:ty) => {
        use num::Zero;
        use approx::AbsDiffEq;
        #[allow(unused_imports)]
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn test_zero(a in any::<$target>()) {
                let z = a.clone() - a;
                prop_assert!(z.is_zero());
            }

            #[test]
            fn test_scalar_distributive(x in any::<$target>(), y in any::<$target>(), a in -1.0..1.0_f64) {
                let a_xy = a * (x.clone() + y.clone());
                let ax_ay = a * x + a * y;
                prop_assert!(a_xy.abs_diff_eq(&ax_ay, crate::ATol::default()));
            }

            #[test]
            fn test_add_associativity(a in any::<$target>(), b in any::<$target>(), c in any::<$target>()) {
                let ab = a.clone() + b.clone();
                let ab_c = ab.clone() + c.clone();
                let bc = b.clone() + c.clone();
                let a_bc = a.clone() + bc.clone();
                prop_assert!(ab_c.abs_diff_eq(&a_bc, crate::ATol::default()), r#"
                    a = {a:?}
                    b = {b:?}
                    c = {c:?}
                    a+b = {ab:?}
                    b+c = {bc:?}
                    (a+b)+c = {ab_c:?}
                    a+(b+c) = {a_bc:?}
                "#);
            }

            #[test]
            fn test_mul_associativity(a in any::<$target>(), b in any::<$target>(), c in any::<$target>()) {
                let ab = a.clone() * b.clone();
                let ab_c = ab.clone() * c.clone();
                let bc = b * c;
                let a_bc = a * bc.clone();
                prop_assert!(a_bc.abs_diff_eq(&ab_c, crate::ATol::default()), r#"
                    a*b = {ab:?}
                    b*c = {bc:?}
                    (a*b)*c = {ab_c:?}
                    a*(b*c) = {a_bc:?}
                "#);
            }

            #[test]
            fn test_distributive(a in any::<$target>(), b in any::<$target>(), c in any::<$target>()) {
                let bc = b.clone() + c.clone();
                let a_bc = a.clone() * bc.clone();
                let ab = a.clone() * b.clone();
                let ac = a.clone() * c.clone();
                let ab_ac = ab.clone() + ac.clone();
                prop_assert!(a_bc.abs_diff_eq(&ab_ac, crate::ATol::default()), r#"
                    a = {a:?}
                    b = {b:?}
                    c = {c:?}
                    b+c = {bc:?}
                    a*b = {ab:?}
                    a*c = {ac:?}
                    a*(b+c) = {a_bc:?}
                    a*b+a*c = {ab_ac:?}
                "#);
            }
        }
    };
}

#[cfg(test)]
pub(crate) use test_algebraic;
