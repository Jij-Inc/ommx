use crate::function::{Atom, Expression, Function, Instruction};
use crate::logical_memory::{LogicalMemoryProfile, LogicalMemoryVisitor, Path};
use std::mem::size_of;

impl LogicalMemoryProfile for Atom {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V) {
        match self {
            Atom::Zero => visitor.visit_leaf(&path.with("Zero"), size_of::<Atom>()),
            Atom::Constant(_) => visitor.visit_leaf(&path.with("Constant"), size_of::<Atom>()),
            Atom::Linear(linear) => {
                linear.visit_logical_memory(path.with("Linear").as_mut(), visitor)
            }
            Atom::Quadratic(quadratic) => {
                quadratic.visit_logical_memory(path.with("Quadratic").as_mut(), visitor)
            }
            Atom::Polynomial(polynomial) => {
                polynomial.visit_logical_memory(path.with("Polynomial").as_mut(), visitor)
            }
        }
    }
}

impl LogicalMemoryProfile for Instruction {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V) {
        match self {
            Instruction::Push(atom) => {
                atom.visit_logical_memory(path.with("Push").as_mut(), visitor)
            }
            Instruction::Unary(_) => {
                visitor.visit_leaf(&path.with("Unary"), size_of::<Instruction>())
            }
            Instruction::Associative(_) => {
                visitor.visit_leaf(&path.with("Associative"), size_of::<Instruction>())
            }
            Instruction::Binary(_) => {
                visitor.visit_leaf(&path.with("Binary"), size_of::<Instruction>())
            }
        }
    }
}

impl LogicalMemoryProfile for Expression {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V) {
        visitor.visit_leaf(&path.with("Vec[stack]"), size_of::<Vec<Instruction>>());
        for instruction in self.instructions() {
            instruction.visit_logical_memory(path, visitor);
        }
    }
}

impl LogicalMemoryProfile for Function {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V) {
        match self {
            Function::Zero => {
                // Zero variant has no heap allocation, only stack size
                visitor.visit_leaf(&path.with("Zero"), size_of::<Function>());
            }
            Function::Constant(_c) => {
                // Constant variant has coefficient on stack, no heap allocation
                visitor.visit_leaf(&path.with("Constant"), size_of::<Function>());
            }
            Function::Linear(linear) => {
                linear.visit_logical_memory(path.with("Linear").as_mut(), visitor);
            }
            Function::Quadratic(quadratic) => {
                quadratic.visit_logical_memory(path.with("Quadratic").as_mut(), visitor);
            }
            Function::Polynomial(polynomial) => {
                polynomial.visit_logical_memory(path.with("Polynomial").as_mut(), visitor);
            }
            Function::Expression(expression) => {
                expression.visit_logical_memory(path.with("Expression").as_mut(), visitor);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logical_memory::logical_memory_to_folded;
    use crate::{coeff, linear, quadratic};

    #[test]
    fn test_function_zero_snapshot() {
        let func = Function::Zero;
        let folded = logical_memory_to_folded(&func);
        insta::assert_snapshot!(folded, @"Zero 40");
    }

    #[test]
    fn test_function_constant_snapshot() {
        let func = Function::Constant(coeff!(42.0));
        let folded = logical_memory_to_folded(&func);
        insta::assert_snapshot!(folded, @"Constant 40");
    }

    #[test]
    fn test_function_linear_snapshot() {
        let func = Function::Linear(
            ((coeff!(2.0) * linear!(1)).unwrap() + (coeff!(3.0) * linear!(2)).unwrap()).unwrap(),
        );
        let folded = logical_memory_to_folded(&func);
        insta::assert_snapshot!(folded, @"Linear;PolynomialBase.terms 80");
    }

    #[test]
    fn test_function_quadratic_snapshot() {
        let func = Function::Quadratic(
            ((coeff!(1.0) * quadratic!(1, 2)).unwrap() + (coeff!(2.0) * quadratic!(1)).unwrap())
                .unwrap(),
        );
        let folded = logical_memory_to_folded(&func);
        insta::assert_snapshot!(folded, @"Quadratic;PolynomialBase.terms 96");
    }

    #[test]
    fn expression_profile_visits_program_and_instruction_payloads() {
        let expression = Function::binary_expression(
            crate::BinaryOperator::Div,
            Function::associative_expression(
                crate::AssociativeOperator::Min,
                Function::from(linear!(1)).abs(),
                Function::one(),
            ),
            Function::from(coeff!(2.0)),
        );
        let folded = logical_memory_to_folded(&expression);

        assert!(folded.contains("Expression;Vec[stack]"));
        assert!(folded.contains("Expression;Push;Linear;PolynomialBase.terms"));
        assert!(folded.contains("Expression;Push;Constant"));
        assert!(folded.contains("Expression;Unary"));
        assert!(folded.contains("Expression;Associative"));
        assert!(folded.contains("Expression;Binary"));
    }
}
