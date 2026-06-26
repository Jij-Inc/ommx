use super::*;

impl Instance {
    /// Reduce binary powers in the instance.
    ///
    /// This method replaces binary powers in the instance with their equivalent linear expressions.
    /// For binary variables, x^n = x for any n >= 1, so we can reduce higher powers to linear terms.
    ///
    /// Returns `true` if any reduction was performed, `false` otherwise.
    pub fn reduce_binary_power(&mut self) -> Result<bool, crate::CoefficientError> {
        let binary_ids = self.binary_ids();
        if binary_ids.is_empty() {
            return Ok(false);
        }
        let mut changed = false;
        let mut updated = self.clone();
        changed |= updated.objective.reduce_binary_power(&binary_ids)?;
        for constraint in updated.constraint_collection.active_mut().values_mut() {
            changed |= constraint.reduce_binary_power(&binary_ids)?;
        }
        // Note: We don't need to reduce in removed_constraints since they are not active
        *self = updated;
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coeff, constraint::CreatedData, quadratic, Bound, Coefficient, Constraint,
        DecisionVariable, Equality, Kind, Sense,
    };
    use ::approx::assert_abs_diff_eq;
    use proptest::prelude::*;

    #[test]
    fn test_instance_reduce_binary_power() {
        // Create instance with binary and continuous variables
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(VariableID::from(1), DecisionVariable::binary());
        decision_variables.insert(
            VariableID::from(2),
            DecisionVariable::new(
                Kind::Continuous,
                Bound::new(0.0, 10.0).unwrap(),
                crate::ATol::default(),
            )
            .unwrap(),
        );

        // Objective: x1^2 + x1*x2 + x2^2
        let mut objective_poly =
            (quadratic!(1, 1) + (coeff!(2.0) * quadratic!(1, 2)).unwrap()).unwrap();
        objective_poly = (objective_poly + (coeff!(3.0) * quadratic!(2, 2)).unwrap()).unwrap();
        let objective = Function::Quadratic(objective_poly);

        // Constraint: x1^2 + x2 <= 5  (i.e., x1^2 + x2 - 5 <= 0)
        let mut constraints = BTreeMap::new();
        let constraint_func = Function::Quadratic(
            ((quadratic!(1, 1) + quadratic!(2)).unwrap() + coeff!(-5.0)).unwrap(),
        );
        constraints.insert(
            ConstraintID::from(1),
            Constraint {
                equality: Equality::LessThanOrEqualToZero,
                stage: CreatedData {
                    function: constraint_func,
                },
            },
        );

        let mut instance =
            Instance::new(Sense::Minimize, objective, decision_variables, constraints).unwrap();

        // Apply reduction
        let changed = instance.reduce_binary_power().unwrap();
        assert!(changed);

        // Check objective: x1^2 -> x1
        let mut expected_objective_poly =
            (quadratic!(1) + (coeff!(2.0) * quadratic!(1, 2)).unwrap()).unwrap();
        expected_objective_poly =
            (expected_objective_poly + (coeff!(3.0) * quadratic!(2, 2)).unwrap()).unwrap();
        let expected_objective = Function::Quadratic(expected_objective_poly);
        assert_abs_diff_eq!(instance.objective(), &expected_objective);

        // Check constraint: x1^2 -> x1
        let expected_constraint_func =
            Function::Quadratic(((quadratic!(1) + quadratic!(2)).unwrap() + coeff!(-5.0)).unwrap());
        assert_eq!(
            instance
                .constraints()
                .get(&ConstraintID::from(1))
                .unwrap()
                .function(),
            &expected_constraint_func
        );

        // Apply reduction again - should not change
        let changed2 = instance.reduce_binary_power().unwrap();
        assert!(!changed2);
    }

    #[test]
    fn test_instance_reduce_binary_power_no_binary() {
        // Create instance with only continuous variables
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(VariableID::from(1), DecisionVariable::continuous());
        decision_variables.insert(VariableID::from(2), DecisionVariable::integer());

        let objective = Function::Quadratic(
            (quadratic!(1, 1) + (coeff!(2.0) * quadratic!(2, 2)).unwrap()).unwrap(),
        );

        let mut instance = Instance::new(
            Sense::Minimize,
            objective.clone(),
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();

        // Apply reduction - should not change since no binary variables
        let changed = instance.reduce_binary_power().unwrap();
        assert!(!changed);
        assert_eq!(instance.objective(), &objective);
    }

    #[test]
    fn reduce_binary_power_preserves_instance_on_coefficient_error() {
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(VariableID::from(1), DecisionVariable::binary());

        let objective = Function::Quadratic(quadratic!(1, 1).into());
        let huge = Coefficient::try_from(f64::MAX).unwrap();
        let overflowing_constraint = Function::Quadratic(
            ((huge * quadratic!(1, 1)).unwrap() + (huge * quadratic!(1)).unwrap()).unwrap(),
        );
        let mut constraints = BTreeMap::new();
        constraints.insert(
            ConstraintID::from(1),
            Constraint {
                equality: Equality::LessThanOrEqualToZero,
                stage: CreatedData {
                    function: overflowing_constraint,
                },
            },
        );
        let mut instance =
            Instance::new(Sense::Minimize, objective, decision_variables, constraints).unwrap();
        let before_objective = instance.objective().clone();
        let before_constraints = instance.constraints().clone();

        let err = instance.reduce_binary_power().unwrap_err();

        assert_eq!(err, crate::CoefficientError::Infinite);
        assert_eq!(instance.objective(), &before_objective);
        assert_eq!(instance.constraints(), &before_constraints);
    }

    proptest! {
        #[test]
        fn test_instance_reduce_binary_power_idempotent(
            mut instance in Instance::arbitrary()
        ) {
            let _first = instance.reduce_binary_power().unwrap();
            let second = instance.reduce_binary_power().unwrap();
            prop_assert!(!second, "reduce_binary_power should be idempotent");
        }
    }
}
