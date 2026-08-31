#[cfg(test)]
use super::operation::{
    associative_expression, binary_expression, AssociativeOperator, BinaryOperator,
};
use super::operation::{from_instructions_exact, instructions, Atom, Instruction, UnaryOperator};
use super::*;
use crate::{
    CoefficientError, Monomial, MonomialDyn, PolynomialBase, QuadraticMonomial, VariableIDSet,
};

impl Function {
    /// Reduce binary powers in the function.
    ///
    /// For binary variables, x^n = x for any n >= 1, so we can reduce higher powers to linear terms.
    ///
    /// Returns `true` if any reduction was performed, `false` otherwise.
    pub fn reduce_binary_power(
        &mut self,
        binary_ids: &VariableIDSet,
    ) -> Result<bool, CoefficientError> {
        let Some(replacement) = self.plan_binary_power_reduction(binary_ids)? else {
            return Ok(false);
        };
        *self = replacement;
        Ok(true)
    }

    /// Build a replacement without mutating the source, returning `None` when
    /// reduction is a no-op.
    ///
    /// Crate-internal: `Instance` needs this borrowed plan across the Function
    /// owner boundary so all fallible rewrites can finish before its commit.
    pub(crate) fn plan_binary_power_reduction(
        &self,
        binary_ids: &VariableIDSet,
    ) -> Result<Option<Self>, CoefficientError> {
        match self {
            Function::Zero | Function::Constant(_) | Function::Linear(_) => Ok(None),
            Function::Quadratic(quadratic) => plan_polynomial_reduction(quadratic, binary_ids)
                .map(|replacement| replacement.map(Function::Quadratic)),
            Function::Polynomial(polynomial) => plan_polynomial_reduction(polynomial, binary_ids)
                .map(|replacement| replacement.map(Function::Polynomial)),
            Function::Expression(expression) => plan_expression_reduction(expression, binary_ids),
        }
    }
}

#[derive(Clone, Copy)]
struct OperandShape {
    is_binary_variable: bool,
}

trait BinaryPowerReducible: Monomial {
    fn has_reducible_binary_power(&self, binary_ids: &VariableIDSet) -> bool;
}

impl BinaryPowerReducible for QuadraticMonomial {
    fn has_reducible_binary_power(&self, binary_ids: &VariableIDSet) -> bool {
        self.as_quadratic()
            .is_some_and(|pair| pair.lower() == pair.upper() && binary_ids.contains(&pair.lower()))
    }
}

impl BinaryPowerReducible for MonomialDyn {
    fn has_reducible_binary_power(&self, binary_ids: &VariableIDSet) -> bool {
        let mut ids = self.ids();
        let Some(mut previous) = ids.next() else {
            return false;
        };
        ids.any(|id| {
            let reducible = id == previous && binary_ids.contains(&id);
            previous = id;
            reducible
        })
    }
}

fn plan_polynomial_reduction<M: BinaryPowerReducible>(
    polynomial: &PolynomialBase<M>,
    binary_ids: &VariableIDSet,
) -> Result<Option<PolynomialBase<M>>, CoefficientError> {
    let has_reducible_power = polynomial
        .keys()
        .any(|monomial| monomial.has_reducible_binary_power(binary_ids));
    if !has_reducible_power {
        return Ok(None);
    }

    let mut replacement = PolynomialBase::default();
    for (monomial, coefficient) in polynomial.iter() {
        let mut reduced = monomial.clone();
        reduced.reduce_binary_power(binary_ids);
        replacement.add_term(reduced, *coefficient)?;
    }
    Ok(Some(replacement))
}

fn plan_expression_reduction(
    expression: &super::operation::Expression,
    binary_ids: &VariableIDSet,
) -> Result<Option<Function>, CoefficientError> {
    let source = instructions(expression);
    let mut replacement: Option<Vec<Instruction>> = None;
    let mut operands = Vec::new();

    for (index, instruction) in source.iter().enumerate() {
        match instruction {
            Instruction::Push(atom) => {
                let reduced_atom = plan_atom_reduction(atom, binary_ids)?;
                let is_binary_variable =
                    is_binary_variable(reduced_atom.as_ref().unwrap_or(atom), binary_ids);

                if let Some(atom) = reduced_atom {
                    replacement
                        .get_or_insert_with(|| source[..index].to_vec())
                        .push(Instruction::Push(atom));
                } else if let Some(replacement) = replacement.as_mut() {
                    replacement.push(instruction.clone());
                }
                operands.push(OperandShape { is_binary_variable });
            }
            Instruction::Unary(UnaryOperator::Powi(exponent))
                if *exponent >= 1
                    && operands
                        .last()
                        .is_some_and(|operand| operand.is_binary_variable) =>
            {
                // The operand program is already present in the replacement.
                // Omitting this instruction implements x^n = x.
                replacement.get_or_insert_with(|| source[..index].to_vec());
            }
            Instruction::Unary(operator) => {
                let _operand = operands
                    .pop()
                    .expect("validated unary instruction has an operand");
                if let Some(replacement) = replacement.as_mut() {
                    replacement.push(Instruction::Unary(*operator));
                }
                operands.push(OperandShape {
                    is_binary_variable: false,
                });
            }
            Instruction::Associative(operator) => {
                combine_operand_shapes(&mut operands);
                if let Some(replacement) = replacement.as_mut() {
                    replacement.push(Instruction::Associative(*operator));
                }
            }
            Instruction::Binary(operator) => {
                combine_operand_shapes(&mut operands);
                if let Some(replacement) = replacement.as_mut() {
                    replacement.push(Instruction::Binary(*operator));
                }
            }
        }
    }

    debug_assert_eq!(operands.len(), 1);
    Ok(replacement.map(|instructions| {
        from_instructions_exact(instructions)
            .expect("reducing atoms and removing unary operations preserves expression validity")
    }))
}

fn plan_atom_reduction(
    atom: &Atom,
    binary_ids: &VariableIDSet,
) -> Result<Option<Atom>, CoefficientError> {
    match atom {
        Atom::Zero | Atom::Constant(_) | Atom::Linear(_) => Ok(None),
        Atom::Quadratic(quadratic) => plan_polynomial_reduction(quadratic, binary_ids)
            .map(|replacement| replacement.map(Atom::Quadratic)),
        Atom::Polynomial(polynomial) => plan_polynomial_reduction(polynomial, binary_ids)
            .map(|replacement| replacement.map(Atom::Polynomial)),
    }
}

fn combine_operand_shapes(operands: &mut Vec<OperandShape>) {
    let _rhs = operands
        .pop()
        .expect("validated binary instruction has a right operand");
    let _lhs = operands
        .pop()
        .expect("validated binary instruction has a left operand");
    operands.push(OperandShape {
        is_binary_variable: false,
    });
}

fn is_binary_variable(atom: &Atom, binary_ids: &VariableIDSet) -> bool {
    match atom {
        Atom::Zero | Atom::Constant(_) => false,
        Atom::Linear(polynomial) => is_binary_variable_polynomial(polynomial, binary_ids),
        Atom::Quadratic(polynomial) => is_binary_variable_polynomial(polynomial, binary_ids),
        Atom::Polynomial(polynomial) => is_binary_variable_polynomial(polynomial, binary_ids),
    }
}

fn is_binary_variable_polynomial<M: Monomial>(
    polynomial: &PolynomialBase<M>,
    binary_ids: &VariableIDSet,
) -> bool {
    if polynomial.constant_term() != 0.0 || polynomial.degree() != 1u32 {
        return false;
    }
    let mut terms = polynomial.linear_terms();
    matches!(
        (terms.next(), terms.next()),
        (Some((id, coefficient)), None)
            if binary_ids.contains(&id) && coefficient == crate::Coefficient::one()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, quadratic, Coefficient};
    use ::approx::assert_abs_diff_eq;

    #[test]
    fn test_function_reduce_binary_power() {
        let binary_ids = crate::variable_ids!(1);

        // Test Zero function
        let mut f = Function::Zero;
        assert!(!f.reduce_binary_power(&binary_ids).unwrap());

        // Test Constant function
        let mut f = Function::Constant(coeff!(5.0));
        assert!(!f.reduce_binary_power(&binary_ids).unwrap());

        // Test Quadratic function with binary variable
        let mut f = Function::Quadratic(
            (quadratic!(1, 1) + (coeff!(2.0) * quadratic!(1, 2)).unwrap()).unwrap(),
        );
        assert!(f.reduce_binary_power(&binary_ids).unwrap());

        let expected = Function::Quadratic(
            (quadratic!(1) + (coeff!(2.0) * quadratic!(1, 2)).unwrap()).unwrap(),
        );
        assert_abs_diff_eq!(f, expected);
    }

    #[test]
    fn no_op_reduction_preserves_composite_expression_shape() {
        let operand = Function::from((coeff!(1e150) * crate::linear!(1)).unwrap());
        let mut product =
            associative_expression(AssociativeOperator::Mul, operand.clone(), operand.clone());
        let original_product = product.clone();
        assert!(!product.reduce_binary_power(&VariableIDSet::new()).unwrap());
        assert!(product == original_product);

        let mut quotient = binary_expression(
            BinaryOperator::Div,
            operand,
            Function::try_from(2.0).unwrap(),
        );
        let original_quotient = quotient.clone();
        assert!(!quotient.reduce_binary_power(&VariableIDSet::new()).unwrap());
        assert!(quotient == original_quotient);
    }

    #[test]
    fn borrowed_plan_returns_none_for_unchanged_functions() {
        let binary_ids = crate::variable_ids!(1);
        let compact = Function::from(quadratic!(2, 2));
        assert!(compact
            .plan_binary_power_reduction(&binary_ids)
            .unwrap()
            .is_none());

        let expression = Function::from(crate::linear!(2)).abs();
        assert!(matches!(expression, Function::Expression(_)));
        assert!(expression
            .plan_binary_power_reduction(&binary_ids)
            .unwrap()
            .is_none());
    }

    #[test]
    fn expression_reduction_preserves_source_on_coefficient_error() {
        let huge = Coefficient::try_from(f64::MAX).unwrap();
        let atom = Function::Quadratic(
            ((huge * quadratic!(1, 1)).unwrap() + (huge * quadratic!(1)).unwrap()).unwrap(),
        );
        let mut expression = atom.abs();
        let original = expression.clone();

        let error = expression
            .reduce_binary_power(&crate::variable_ids!(1))
            .unwrap_err();

        assert_eq!(error, CoefficientError::Infinite);
        assert_eq!(expression, original);
    }

    #[test]
    fn integer_power_node_reduces_for_a_binary_variable() {
        let binary_ids = crate::variable_ids!(1);
        let mut function = Function::from(crate::linear!(1)).powi(2);

        assert!(function.reduce_binary_power(&binary_ids).unwrap());
        assert!(function == Function::from(crate::linear!(1)));
    }

    #[test]
    fn nested_integer_power_nodes_reduce_iteratively() {
        let binary_ids = crate::variable_ids!(1);
        let mut function = Function::from(crate::linear!(1)).powi(2).powi(3);

        assert!(function.reduce_binary_power(&binary_ids).unwrap());
        assert!(function == Function::from(crate::linear!(1)));
    }
}
