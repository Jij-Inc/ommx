use super::*;

impl Instance {
    /// Reduce binary powers in the instance.
    ///
    /// This method replaces binary powers in the instance with their equivalent linear expressions.
    /// For binary variables, x^n = x for any n >= 1, so we can reduce higher powers to linear terms.
    ///
    /// Returns `true` if any reduction was performed, `false` otherwise.
    pub fn reduce_binary_power(&mut self) -> bool {
        let binary_ids = self.binary_ids();
        if binary_ids.is_empty() {
            return false;
        }
        let mut changed = false;
        changed |= self.objective.reduce_binary_power(&binary_ids);
        for constraint in self.constraints.values_mut() {
            changed |= constraint.reduce_binary_power(&binary_ids);
        }
        // Note: We don't need to reduce in removed_constraints since they are not active
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coeff, quadratic, ATol, Bound, Constraint, DecisionVariable, Equality, Kind, Sense,
    };
    use ::approx::assert_abs_diff_eq;
    use fnv::FnvHashMap;
    use proptest::prelude::*;

    #[test]
    fn test_instance_reduce_binary_power() {
        // Create instance with binary and continuous variables
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(
            VariableID::from(1),
            DecisionVariable::binary(VariableID::from(1)),
        );
        decision_variables.insert(
            VariableID::from(2),
            DecisionVariable::new(
                VariableID::from(2),
                Kind::Continuous,
                Bound::new(0.0, 10.0).unwrap(),
                None,
                ATol::default(),
            )
            .unwrap(),
        );

        // Objective: x1^2 + x1*x2 + x2^2
        let objective = Function::Quadratic(
            quadratic!(1, 1) + coeff!(2.0) * quadratic!(1, 2) + coeff!(3.0) * quadratic!(2, 2),
        );

        // Constraint: x1^2 + x2 <= 5  (i.e., x1^2 + x2 - 5 <= 0)
        let mut constraints = BTreeMap::new();
        let constraint_func = Function::Quadratic(quadratic!(1, 1) + quadratic!(2) + coeff!(-5.0));
        constraints.insert(
            ConstraintID::from(1),
            Constraint {
                id: ConstraintID::from(1),
                function: constraint_func,
                equality: Equality::LessThanOrEqualToZero,
                name: None,
                subscripts: vec![],
                parameters: FnvHashMap::default(),
                description: None,
            },
        );

        let mut instance =
            Instance::new(Sense::Minimize, objective, decision_variables, constraints).unwrap();

        // Apply reduction
        let changed = instance.reduce_binary_power();
        assert!(changed);

        // Check objective: x1^2 -> x1
        let expected_objective = Function::Quadratic(
            quadratic!(1) + coeff!(2.0) * quadratic!(1, 2) + coeff!(3.0) * quadratic!(2, 2),
        );
        assert_abs_diff_eq!(instance.objective(), &expected_objective);

        // Check constraint: x1^2 -> x1
        let expected_constraint_func =
            Function::Quadratic(quadratic!(1) + quadratic!(2) + coeff!(-5.0));
        assert_eq!(
            &instance
                .constraints()
                .get(&ConstraintID::from(1))
                .unwrap()
                .function,
            &expected_constraint_func
        );

        // Apply reduction again - should not change
        let changed2 = instance.reduce_binary_power();
        assert!(!changed2);
    }

    #[test]
    fn test_instance_reduce_binary_power_no_binary() {
        // Create instance with only continuous variables
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(
            VariableID::from(1),
            DecisionVariable::continuous(VariableID::from(1)),
        );
        decision_variables.insert(
            VariableID::from(2),
            DecisionVariable::integer(VariableID::from(2)),
        );

        let objective = Function::Quadratic(quadratic!(1, 1) + coeff!(2.0) * quadratic!(2, 2));

        let mut instance = Instance::new(
            Sense::Minimize,
            objective.clone(),
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();

        // Apply reduction - should not change since no binary variables
        let changed = instance.reduce_binary_power();
        assert!(!changed);
        assert_eq!(instance.objective(), &objective);
    }

    proptest! {
        #[test]
        fn test_instance_reduce_binary_power_idempotent(
            mut instance in Instance::arbitrary()
        ) {
            let _first = instance.reduce_binary_power();
            let second = instance.reduce_binary_power();
            prop_assert!(!second, "reduce_binary_power should be idempotent");
        }
    }
}
