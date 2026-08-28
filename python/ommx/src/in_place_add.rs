//! Atomicity guard for Python in-place addition.
//!
//! The Rust SDK's in-place addition primitives intentionally allow partial
//! mutation on coefficient-arithmetic failure. Python `+=` first checks only
//! the overlapping coefficients, so it can keep the receiver unchanged on
//! error without cloning the growing accumulator on every successful call.

use ommx::{Coefficient, CoefficientError, Function, Monomial, PolynomialBase};

pub fn preflight_polynomial<M, N>(
    lhs: &PolynomialBase<M>,
    rhs: &PolynomialBase<N>,
) -> Result<(), CoefficientError>
where
    M: Monomial + From<N>,
    N: Monomial,
{
    for (monomial, rhs_coefficient) in rhs.iter() {
        let monomial = M::from(monomial.clone());
        if let Some(lhs_coefficient) = lhs.get(&monomial) {
            let _ = (lhs_coefficient + *rhs_coefficient)?;
        }
    }
    Ok(())
}

fn preflight_constant<M: Monomial>(
    polynomial: &PolynomialBase<M>,
    constant: Coefficient,
) -> Result<(), CoefficientError> {
    if let Some(existing) = polynomial.get(&M::default()) {
        let _ = (existing + constant)?;
    }
    Ok(())
}

pub fn preflight_function(lhs: &Function, rhs: &Function) -> Result<(), CoefficientError> {
    match (lhs, rhs) {
        (Function::Zero, _) | (_, Function::Zero) => Ok(()),
        (Function::Constant(lhs), Function::Constant(rhs)) => {
            let _ = (*lhs + *rhs)?;
            Ok(())
        }
        (Function::Constant(constant), Function::Linear(linear))
        | (Function::Linear(linear), Function::Constant(constant)) => {
            preflight_constant(linear, *constant)
        }
        (Function::Constant(constant), Function::Quadratic(quadratic))
        | (Function::Quadratic(quadratic), Function::Constant(constant)) => {
            preflight_constant(quadratic, *constant)
        }
        (Function::Constant(constant), Function::Polynomial(polynomial))
        | (Function::Polynomial(polynomial), Function::Constant(constant)) => {
            preflight_constant(polynomial, *constant)
        }
        (Function::Linear(lhs), Function::Linear(rhs)) => preflight_polynomial(lhs, rhs),
        (Function::Linear(linear), Function::Quadratic(quadratic))
        | (Function::Quadratic(quadratic), Function::Linear(linear)) => {
            preflight_polynomial(quadratic, linear)
        }
        (Function::Linear(linear), Function::Polynomial(polynomial))
        | (Function::Polynomial(polynomial), Function::Linear(linear)) => {
            preflight_polynomial(polynomial, linear)
        }
        (Function::Quadratic(lhs), Function::Quadratic(rhs)) => preflight_polynomial(lhs, rhs),
        (Function::Quadratic(quadratic), Function::Polynomial(polynomial))
        | (Function::Polynomial(polynomial), Function::Quadratic(quadratic)) => {
            preflight_polynomial(polynomial, quadratic)
        }
        (Function::Polynomial(lhs), Function::Polynomial(rhs)) => preflight_polynomial(lhs, rhs),
    }
}
