//! Additional trait implementations for generated codes

/// Creates a [`Coefficient`] from a floating-point literal.
///
/// This macro is a convenience wrapper around `Coefficient::try_from().unwrap()`
/// for use with compile-time known floating-point literals. It should only be used
/// when the value is guaranteed to be valid (non-zero, finite, not NaN).
///
/// # Panics
///
/// Panics if the literal value is zero, infinite, or NaN.
///
/// # Examples
///
/// ```
/// use ommx::{coeff, linear, LinearMonomial, VariableID};
///
/// // Create coefficients from literals
/// let c1 = coeff!(2.5);
/// let c2 = coeff!(-1.0);
/// let c3 = coeff!(0.5);
///
/// // Use in expressions
/// let expr = c1 * linear!(1)
///     + c2 * linear!(2);
/// ```
///
/// # Note
///
/// For runtime values or when error handling is needed, use `Coefficient::try_from()` instead:
///
/// ```
/// use ommx::Coefficient;
///
/// let runtime_value = 3.14;
/// let coeff = Coefficient::try_from(runtime_value)?;
/// # Ok::<(), ommx::CoefficientError>(())
/// ```
#[macro_export]
macro_rules! coeff {
    ($literal:literal) => {
        $crate::Coefficient::try_from($literal).unwrap()
    };
}

/// Creates a [`LinearMonomial`] from a variable ID literal.
///
/// This macro is a convenience wrapper for creating linear monomials from integer literals
/// representing variable IDs.
///
/// # Examples
///
/// ```
/// use ommx::{linear, LinearMonomial, VariableID};
///
/// // Create a linear monomial for variable x1
/// let x1 = linear!(1);
/// assert_eq!(x1, LinearMonomial::Variable(VariableID::from(1)));
/// ```
#[macro_export]
macro_rules! linear {
    ($id:literal) => {
        $crate::LinearMonomial::Variable($crate::VariableID::from($id))
    };
}

/// Creates a [`QuadraticMonomial`] from variable ID literals.
///
/// This macro supports creating quadratic monomials in multiple forms:
/// - `quadratic!(id)` creates a linear term within the quadratic space
/// - `quadratic!(id1, id2)` creates a quadratic pair term
///
/// # Examples
///
/// ```
/// use ommx::{quadratic, QuadraticMonomial, VariableID};
///
/// // Create a linear term in quadratic space (x1)
/// let x1 = quadratic!(1);
/// assert_eq!(x1, QuadraticMonomial::Linear(VariableID::from(1)));
///
/// // Create a quadratic pair term (x1 * x2)
/// let x1_x2 = quadratic!(1, 2);
/// assert_eq!(x1_x2, QuadraticMonomial::new_pair(VariableID::from(1), VariableID::from(2)));
/// ```
#[macro_export]
macro_rules! quadratic {
    ($id:literal) => {
        $crate::QuadraticMonomial::Linear($crate::VariableID::from($id))
    };
    ($id1:literal, $id2:literal) => {
        $crate::QuadraticMonomial::new_pair(
            $crate::VariableID::from($id1),
            $crate::VariableID::from($id2),
        )
    };
}

/// Creates a [`MonomialDyn`] from variable ID literals.
///
/// This macro creates a general monomial from one or more variable ID literals.
/// The degree of the monomial depends on the number of variables provided.
///
/// # Examples
///
/// ```
/// use ommx::{monomial, MonomialDyn, VariableID};
///
/// // Create a linear monomial (x1)
/// let x1 = monomial!(1);
///
/// // Create a quadratic monomial (x1 * x2)
/// let x1_x2 = monomial!(1, 2);
///
/// // Create a cubic monomial (x1 * x2 * x3)
/// let x1_x2_x3 = monomial!(1, 2, 3);
/// ```
#[macro_export]
macro_rules! monomial {
    ($($id:literal),+) => {
        $crate::MonomialDyn::new(vec![$($crate::VariableID::from($id)),+])
    };
}

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
