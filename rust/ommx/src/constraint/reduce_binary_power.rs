use super::*;
use crate::VariableIDSet;

impl Constraint<Created> {
    /// Reduce binary powers in the constraint function.
    ///
    /// For binary variables, x^n = x for any n >= 1, so we can reduce higher powers to linear terms.
    ///
    /// Returns `true` if any reduction was performed, `false` otherwise.
    pub fn reduce_binary_power(&mut self, binary_ids: &VariableIDSet) -> bool {
        self.stage.function.reduce_binary_power(binary_ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quadratic;

    #[test]
    fn test_constraint_reduce_binary_power() {
        let binary_ids = crate::variable_ids!(1);

        // Create a constraint with x1^2 + x2 <= 0
        let function = Function::Quadratic(quadratic!(1, 1) + quadratic!(2));

        let mut constraint: Constraint<Created> = Constraint {
            equality: Equality::LessThanOrEqualToZero,
            stage: CreatedData { function },
        };

        // Apply reduction
        let changed = constraint.reduce_binary_power(&binary_ids);
        assert!(changed);

        // Check that x1^2 was reduced to x1
        let expected_function = Function::Quadratic(quadratic!(1) + quadratic!(2));
        assert_eq!(constraint.stage.function, expected_function);
    }
}
