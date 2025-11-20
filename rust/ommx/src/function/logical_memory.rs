use crate::function::Function;
use crate::logical_memory::{LogicalMemoryProfile, LogicalMemoryVisitor, Path};
use std::mem::size_of;

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
        let func =
            Function::Quadratic(coeff!(1.0) * quadratic!(1, 2) + coeff!(2.0) * quadratic!(1));
        let folded = logical_memory_to_folded("Function", &func);
        insta::assert_snapshot!(folded, @"Function;Quadratic;terms 128");
    }
}
