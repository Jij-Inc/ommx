use crate::logical_memory::{LogicalMemoryProfile, LogicalMemoryVisitor};
use crate::function::Function;
use std::mem::size_of;

impl LogicalMemoryProfile for Function {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(
        &self,
        path: &mut Vec<&'static str>,
        visitor: &mut V,
    ) {
        match self {
            Function::Zero => {
                // Zero variant has no heap allocation, only stack size
                path.push("Zero");
                visitor.visit_leaf(path, size_of::<Function>());
                path.pop();
            }
            Function::Constant(_c) => {
                // Constant variant has coefficient on stack, no heap allocation
                path.push("Constant");
                visitor.visit_leaf(path, size_of::<Function>());
                path.pop();
            }
            Function::Linear(linear) => {
                path.push("Linear");
                linear.visit_logical_memory(path, visitor);
                path.pop();
            }
            Function::Quadratic(quadratic) => {
                path.push("Quadratic");
                quadratic.visit_logical_memory(path, visitor);
                path.pop();
            }
            Function::Polynomial(polynomial) => {
                path.push("Polynomial");
                polynomial.visit_logical_memory(path, visitor);
                path.pop();
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
        let folded = logical_memory_to_folded("Function", &func);
        insta::assert_snapshot!(folded, @"Function;Zero 40");
    }

    #[test]
    fn test_function_constant_snapshot() {
        let func = Function::Constant(coeff!(42.0));
        let folded = logical_memory_to_folded("Function", &func);
        insta::assert_snapshot!(folded, @"Function;Constant 40");
    }

    #[test]
    fn test_function_linear_snapshot() {
        let func = Function::Linear(coeff!(2.0) * linear!(1) + coeff!(3.0) * linear!(2));
        let folded = logical_memory_to_folded("Function", &func);
        insta::assert_snapshot!(folded, @"Function;Linear;terms 104");
    }

    #[test]
    fn test_function_quadratic_snapshot() {
        let func = Function::Quadratic(coeff!(1.0) * quadratic!(1, 2) + coeff!(2.0) * quadratic!(1));
        let folded = logical_memory_to_folded("Function", &func);
        insta::assert_snapshot!(folded, @"Function;Quadratic;terms 128");
    }
}
