use super::*;
use crate::VariableIDSet;

impl Function {
    /// Reduce binary powers in the function.
    ///
    /// For binary variables, x^n = x for any n >= 1, so we can reduce higher powers to linear terms.
    ///
    /// Returns `true` if any reduction was performed, `false` otherwise.
    pub fn reduce_binary_power(&mut self, binary_ids: &VariableIDSet) -> bool {
        match self {
            Function::Zero => false,
            Function::Constant(_) => false,
            Function::Linear(l) => l.reduce_binary_power(binary_ids),
            Function::Quadratic(q) => q.reduce_binary_power(binary_ids),
            Function::Polynomial(p) => p.reduce_binary_power(binary_ids),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, quadratic};
    use ::approx::assert_abs_diff_eq;

    #[test]
    fn test_function_reduce_binary_power() {
        let mut binary_ids = VariableIDSet::default();
        binary_ids.insert(VariableID::from(1));

        // Test Zero function
        let mut f = Function::Zero;
        assert!(!f.reduce_binary_power(&binary_ids));

        // Test Constant function
        let mut f = Function::Constant(coeff!(5.0));
        assert!(!f.reduce_binary_power(&binary_ids));

        // Test Quadratic function with binary variable
        let mut f =
            Function::Quadratic(coeff!(1.0) * quadratic!(1, 1) + coeff!(2.0) * quadratic!(1, 2));
        assert!(f.reduce_binary_power(&binary_ids));

        let expected =
            Function::Quadratic(coeff!(1.0) * quadratic!(1) + coeff!(2.0) * quadratic!(1, 2));
        assert_abs_diff_eq!(f, expected);
    }
}
