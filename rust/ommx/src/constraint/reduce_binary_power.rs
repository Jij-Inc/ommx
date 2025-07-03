use super::*;
use crate::VariableIDSet;

impl Constraint {
    /// Reduce binary powers in the constraint function.
    ///
    /// For binary variables, x^n = x for any n >= 1, so we can reduce higher powers to linear terms.
    ///
    /// Returns `true` if any reduction was performed, `false` otherwise.
    pub fn reduce_binary_power(&mut self, binary_ids: &VariableIDSet) -> bool {
        self.function.reduce_binary_power(binary_ids)
    }
}

impl RemovedConstraint {
    /// Reduce binary powers in the removed constraint function.
    ///
    /// For binary variables, x^n = x for any n >= 1, so we can reduce higher powers to linear terms.
    ///
    /// Returns `true` if any reduction was performed, `false` otherwise.
    pub fn reduce_binary_power(&mut self, binary_ids: &VariableIDSet) -> bool {
        self.constraint.reduce_binary_power(binary_ids)
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

        let mut constraint = Constraint {
            id: ConstraintID::from(1),
            function,
            equality: Equality::LessThanOrEqualToZero,
            name: None,
            subscripts: vec![],
            parameters: FnvHashMap::default(),
            description: None,
        };

        // Apply reduction
        let changed = constraint.reduce_binary_power(&binary_ids);
        assert!(changed);

        // Check that x1^2 was reduced to x1
        let expected_function = Function::Quadratic(quadratic!(1) + quadratic!(2));
        assert_eq!(constraint.function, expected_function);
    }
}
