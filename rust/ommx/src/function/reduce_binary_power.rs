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
            Function::Expression(_) => {
                // Work on a clone so a coefficient-combination error leaves
                // the original function unchanged.
                let Function::Expression(expression) = self.clone() else {
                    unreachable!("expression variant matched above")
                };
                let mut instructions = Vec::with_capacity(expression.instructions().len());
                let mut operands = Vec::new();
                let mut reduced = false;

                for instruction in expression.into_instructions() {
                    match instruction {
                        Instruction::Push(atom) => {
                            let mut function = atom.into_function();
                            reduced |= function.reduce_binary_power(binary_ids)?;
                            let is_binary_variable = is_binary_variable(&function, binary_ids);
                            let start = instructions.len();
                            instructions.extend(function.into_instructions());
                            operands.push(OperandShape {
                                start,
                                is_binary_variable,
                            });
                        }
                        Instruction::Unary(UnaryOperator::Powi(exponent))
                            if exponent >= 1
                                && operands
                                    .last()
                                    .is_some_and(|operand| operand.is_binary_variable) =>
                        {
                            // The operand program is already present in the
                            // output. Omitting this instruction implements
                            // x^n = x without rebuilding a tree.
                            reduced = true;
                        }
                        Instruction::Unary(operator) => {
                            let operand = operands
                                .pop()
                                .expect("validated unary instruction has an operand");
                            instructions.push(Instruction::Unary(operator));
                            operands.push(OperandShape {
                                start: operand.start,
                                is_binary_variable: false,
                            });
                        }
                        Instruction::Associative(operator) => {
                            combine_operand_shapes(&mut operands);
                            instructions.push(Instruction::Associative(operator));
                        }
                        Instruction::Binary(operator) => {
                            combine_operand_shapes(&mut operands);
                            instructions.push(Instruction::Binary(operator));
                        }
                    }
                }

                debug_assert_eq!(operands.len(), 1);
                let rebuilt = Function::from_instructions_exact(instructions)
                    .expect("removing unary operations preserves expression validity");
                *self = rebuilt;
                Ok(reduced)
            }
        }
    }
}

#[derive(Clone, Copy)]
struct OperandShape {
    /// Start of this operand's contiguous instruction span.
    start: usize,
    is_binary_variable: bool,
}

fn combine_operand_shapes(operands: &mut Vec<OperandShape>) {
    let _rhs = operands
        .pop()
        .expect("validated binary instruction has a right operand");
    let lhs = operands
        .pop()
        .expect("validated binary instruction has a left operand");
    operands.push(OperandShape {
        start: lhs.start,
        is_binary_variable: false,
    });
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
        let mut product = Function::associative_expression(
            AssociativeOperator::Mul,
            operand.clone(),
            operand.clone(),
        );
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

    #[test]
    fn nested_integer_power_nodes_reduce_iteratively() {
        let binary_ids = crate::variable_ids!(1);
        let mut function = Function::from(crate::linear!(1)).powi(2).powi(3);

        assert!(function.reduce_binary_power(&binary_ids).unwrap());
        assert!(function == Function::from(crate::linear!(1)));
    }
}
