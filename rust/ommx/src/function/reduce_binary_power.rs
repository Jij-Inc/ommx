use super::*;
use crate::{CoefficientError, VariableIDSet};

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
        match self {
            Function::Zero | Function::Constant(_) | Function::Linear(_) => Ok(false),
            Function::Quadratic(q) => q.reduce_binary_power(binary_ids),
            Function::Polynomial(p) => p.reduce_binary_power(binary_ids),
            Function::Unary(_) | Function::Nary(_) | Function::Binary(_) => {
                let owned = self.clone();
                let (rebuilt, reduced) = match owned {
                    Function::Unary(operation) => {
                        let (operator, mut operand) = operation.into_parts();
                        let mut reduced = operand.reduce_binary_power(binary_ids)?;
                        if matches!(operator, UnaryOperator::Powi(exponent) if exponent >= 1)
                            && is_binary_variable(&operand, binary_ids)
                        {
                            reduced = true;
                            (operand, reduced)
                        } else {
                            (Function::unary_expression(operator, operand), reduced)
                        }
                    }
                    Function::Nary(operation) => {
                        let (operator, mut operands) = operation.into_parts();
                        let mut reduced = false;
                        for operand in &mut operands {
                            reduced |= operand.reduce_binary_power(binary_ids)?;
                        }
                        (Function::nary_expression(operator, operands), reduced)
                    }
                    Function::Binary(operation) => {
                        let (operator, mut lhs, mut rhs) = operation.into_parts();
                        let reduced = lhs.reduce_binary_power(binary_ids)?
                            | rhs.reduce_binary_power(binary_ids)?;
                        (Function::binary_expression(operator, lhs, rhs), reduced)
                    }
                    _ => unreachable!("composite variants matched above"),
                };
                *self = rebuilt;
                Ok(reduced)
            }
        }
    }
}

fn is_binary_variable(function: &Function, binary_ids: &VariableIDSet) -> bool {
    if function.constant_term() != Some(0.0) || function.degree() != Some(1.into()) {
        return false;
    }
    let Some(mut terms) = function.linear_terms() else {
        return false;
    };
    matches!(
        (terms.next(), terms.next()),
        (Some((id, coefficient)), None)
            if binary_ids.contains(&id) && coefficient == crate::Coefficient::one()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, quadratic};
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
            Function::nary_expression(NaryOperator::Mul, vec![operand.clone(), operand.clone()]);
        let original_product = product.clone();
        assert!(!product.reduce_binary_power(&VariableIDSet::new()).unwrap());
        assert!(product == original_product);

        let mut quotient = Function::binary_expression(
            BinaryOperator::Div,
            operand,
            Function::try_from(2.0).unwrap(),
        );
        let original_quotient = quotient.clone();
        assert!(!quotient.reduce_binary_power(&VariableIDSet::new()).unwrap());
        assert!(quotient == original_quotient);
    }

    #[test]
    fn integer_power_node_reduces_for_a_binary_variable() {
        let binary_ids = crate::variable_ids!(1);
        let mut function = Function::from(crate::linear!(1)).powi(2);

        assert!(function.reduce_binary_power(&binary_ids).unwrap());
        assert!(function == Function::from(crate::linear!(1)));
    }
}
