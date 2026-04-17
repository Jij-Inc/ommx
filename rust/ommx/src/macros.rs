//! Additional trait implementations for generated codes

/// Creates a [`crate::Coefficient`] from a floating-point expression.
///
/// This macro is a convenience wrapper around `Coefficient::try_from().expect()`
/// for use with floating-point values. It accepts both literals and expressions.
///
/// # Panics
///
/// Panics if the value is zero, infinite, or NaN.
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
/// // Create coefficients from expressions
/// let x = 2.0;
/// let c4 = coeff!(x + 0.5);
/// let c5 = coeff!(x * 3.0);
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
    ($expr:expr) => {
        $crate::Coefficient::try_from($expr).expect(concat!(
            "Failed to create Coefficient from expression: ",
            stringify!($expr),
            ". The value must be non-zero, finite, and not NaN"
        ))
    };
}

/// Creates a [`crate::LinearMonomial`] from a variable ID expression.
///
/// This macro is a convenience wrapper for creating linear monomials from integer expressions
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
///
/// // Create from expressions
/// let i = 2;
/// let x2 = linear!(i);
/// let x3 = linear!(i + 1);
/// ```
#[macro_export]
macro_rules! linear {
    ($id:expr) => {
        $crate::LinearMonomial::Variable($crate::VariableID::from($id))
    };
}

/// Creates a [`crate::QuadraticMonomial`] from variable ID expressions.
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
///
/// // Create from expressions
/// let i = 1;
/// let j = 2;
/// let xi_xj = quadratic!(i, j);
/// ```
#[macro_export]
macro_rules! quadratic {
    ($id:expr) => {
        $crate::QuadraticMonomial::Linear($crate::VariableID::from($id))
    };
    ($id1:expr, $id2:expr) => {
        $crate::QuadraticMonomial::new_pair(
            $crate::VariableID::from($id1),
            $crate::VariableID::from($id2),
        )
    };
}

/// Creates a [`crate::MonomialDyn`] from variable ID expressions.
///
/// This macro creates a general monomial from one or more variable ID expressions.
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
///
/// // Create from expressions
/// let i = 1;
/// let j = 2;
/// let k = 3;
/// let xi_xj_xk = monomial!(i, j, k);
/// ```
#[macro_export]
macro_rules! monomial {
    ($($id:expr),+) => {
        $crate::MonomialDyn::new(vec![$($crate::VariableID::from($id)),+])
    };
}

/// Creates a [`crate::VariableIDSet`] from variable ID expressions.
///
/// This macro creates a `VariableIDSet` from one or more variable ID expressions.
/// It's a convenience wrapper for creating binary variable sets for use with
/// binary power reduction operations.
///
/// # Examples
///
/// ```
/// use ommx::{variable_ids, VariableIDSet, VariableID};
///
/// // Create a set containing variable x1
/// let binary_set = variable_ids!(1);
///
/// // Create a set containing variables x1 and x3
/// let binary_set = variable_ids!(1, 3);
///
/// // Create a set containing variables x1, x2, and x5
/// let binary_set = variable_ids!(1, 2, 5);
///
/// // Create from expressions
/// let i = 1;
/// let j = 3;
/// let binary_set = variable_ids!(i, j, i + 4);
/// ```
#[macro_export]
macro_rules! variable_ids {
    ($($id:expr),+) => {
        {
            let mut set = $crate::VariableIDSet::default();
            $(set.insert($crate::VariableID::from($id));)+
            set
        }
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
