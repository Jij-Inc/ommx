use super::operation::{instructions, Instruction, PolynomialRef};
use super::*;
use crate::ATol;
use ::approx::AbsDiffEq;

impl AbsDiffEq for Function {
    type Epsilon = ATol;

    fn default_epsilon() -> Self::Epsilon {
        ATol::default()
    }

    fn abs_diff_eq(&self, other: &Self, epsilon: Self::Epsilon) -> bool {
        if let (Some(lhs), Some(rhs)) = (
            PolynomialRef::from_function(self),
            PolynomialRef::from_function(other),
        ) {
            return polynomial_abs_diff_eq(lhs, rhs, epsilon);
        }

        let (Function::Expression(lhs), Function::Expression(rhs)) = (self, other) else {
            return false;
        };
        instructions(lhs).len() == instructions(rhs).len()
            && instructions(lhs)
                .iter()
                .zip(instructions(rhs))
                .all(|(lhs, rhs)| instruction_abs_diff_eq(lhs, rhs, epsilon))
    }
}

fn polynomial_abs_diff_eq(lhs: PolynomialRef<'_>, rhs: PolynomialRef<'_>, epsilon: ATol) -> bool {
    lhs.all_terms(|monomial, lhs_coefficient| {
        coefficient_abs_diff_eq(Some(lhs_coefficient), rhs.coefficient(monomial), epsilon)
    }) && rhs.all_terms(|monomial, rhs_coefficient| {
        lhs.coefficient(monomial).is_some()
            || coefficient_abs_diff_eq(None, Some(rhs_coefficient), epsilon)
    })
}

fn coefficient_abs_diff_eq(
    lhs: Option<Coefficient>,
    rhs: Option<Coefficient>,
    epsilon: ATol,
) -> bool {
    let lhs = lhs.map_or(0.0, Coefficient::into_inner);
    let rhs = rhs.map_or(0.0, Coefficient::into_inner);
    let difference = lhs - rhs;
    difference.is_finite() && difference.abs() <= epsilon.into_inner()
}

fn instruction_abs_diff_eq(lhs: &Instruction, rhs: &Instruction, epsilon: ATol) -> bool {
    match (lhs, rhs) {
        (Instruction::Push(lhs), Instruction::Push(rhs)) => polynomial_abs_diff_eq(
            PolynomialRef::from_atom(lhs),
            PolynomialRef::from_atom(rhs),
            epsilon,
        ),
        (Instruction::Unary(lhs), Instruction::Unary(rhs)) => lhs == rhs,
        (Instruction::Associative(lhs), Instruction::Associative(rhs)) => lhs == rhs,
        (Instruction::Binary(lhs), Instruction::Binary(rhs)) => lhs == rhs,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, linear, FunctionParameters, Linear, Polynomial, PolynomialParameters};
    use ::approx::{assert_abs_diff_eq, assert_abs_diff_ne};
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn borrowed_polynomial_comparison_matches_subtraction(
            (lhs, rhs) in PolynomialParameters::arbitrary().prop_flat_map(|parameters| {
                let parameters = FunctionParameters::polynomial_only(parameters);
                (
                    Function::arbitrary_with(parameters),
                    Function::arbitrary_with(parameters),
                )
            }),
        ) {
            let epsilon = ATol::default();
            let expected = match lhs.clone() - rhs.clone() {
                Ok(diff) => diff
                    .values()
                    .expect("difference of polynomials is a polynomial")
                    .map(Coefficient::abs)
                    .max()
                    .is_none_or(|coefficient| coefficient <= epsilon),
                Err(_) => false,
            };

            prop_assert_eq!(lhs.abs_diff_eq(&rhs, epsilon), expected);
        }
    }

    #[test]
    fn test_abs_diff_eq() {
        let f = Function::from(coeff!(1.0));
        let g = Function::from(coeff!(1.0000000001));
        assert_abs_diff_eq!(f, g);
    }

    #[test]
    fn expression_atoms_use_coefficient_tolerance() {
        let f = Function::from(coeff!(1.0)).abs();
        let g = Function::from(coeff!(1.0000000001)).abs();
        assert_abs_diff_eq!(f, g);
    }

    #[test]
    fn polynomial_variants_compare_as_coefficient_maps() {
        let linear = Linear::from(linear!(1));
        let polynomial = Polynomial::from(linear.clone());

        assert_abs_diff_eq!(
            Function::Linear(linear.clone()),
            Function::Polynomial(polynomial.clone())
        );
        assert_abs_diff_eq!(
            Function::Linear(linear).abs(),
            Function::Polynomial(polynomial).abs()
        );
    }

    #[test]
    fn expression_comparison_preserves_operator_structure() {
        let f = Function::from(linear!(1));
        assert_abs_diff_ne!(f.clone().abs(), f.signum());
    }

    #[test]
    fn overflow_is_not_equal_even_with_infinite_tolerance() {
        let positive = Function::Constant(Coefficient::try_from(f64::MAX).unwrap());
        let negative = Function::Constant(Coefficient::try_from(-f64::MAX).unwrap());
        let epsilon = ATol::new(f64::INFINITY).unwrap();

        assert!(!positive.abs_diff_eq(&negative, epsilon));
    }
}
