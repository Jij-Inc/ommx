use super::*;

impl Instance {
    /// Validate that all required variable IDs are defined in the instance
    /// and are not dependent variables (i.e., not used as keys in decision_variable_dependency)
    fn validate_required_ids(&self, required_ids: VariableIDSet) -> anyhow::Result<()> {
        let variable_ids: VariableIDSet = self.decision_variables.keys().cloned().collect();
        let dependency_keys: VariableIDSet = self.decision_variable_dependency.keys().collect();

        // Check if all required IDs are defined
        if !required_ids.is_subset(&variable_ids) {
            let undefined_id = required_ids.difference(&variable_ids).next().unwrap();
            return Err(InstanceError::UndefinedVariableID { id: *undefined_id }.into());
        }

        // Check if any required ID is a dependent variable (used as a key in decision_variable_dependency)
        let mut intersection = required_ids.intersection(&dependency_keys);
        if let Some(&id) = intersection.next() {
            return Err(InstanceError::DependentVariableUsed { id }.into());
        }

        Ok(())
    }

    /// Set the objective function
    pub fn set_objective(&mut self, objective: Function) -> anyhow::Result<()> {
        // Validate that all variables in the objective are defined
        self.validate_required_ids(objective.required_ids())?;
        self.objective = objective;
        Ok(())
    }

    /// Insert a constraint into the instance.
    ///
    /// - If the constraint already exists, it will be replaced.
    /// - If the ID of given constraint is in the removed constraints, it replaces it.
    /// - Otherwise, it adds the constraint to the instance.
    ///
    pub fn insert_constraint(
        &mut self,
        constraint: Constraint,
    ) -> anyhow::Result<Option<Constraint>> {
        // Validate that all variables in the constraints are defined
        self.validate_required_ids(constraint.required_ids())?;
        use std::collections::btree_map::Entry;
        if let Entry::Occupied(mut o) = self.removed_constraints.entry(constraint.id) {
            let removed = std::mem::replace(&mut o.get_mut().constraint, constraint);
            return Ok(Some(removed));
        }
        Ok(self.constraints.insert(constraint.id, constraint))
    }

    /// Generate a new unique ConstraintID.
    ///
    /// Returns the next available constraint ID by finding the maximum ID
    /// from both active constraints and removed constraints, then adding 1.
    /// If there are no constraints, returns ConstraintID(1).
    ///
    /// Note: This method does not track which IDs have been allocated.
    /// Consecutive calls will return the same ID until a constraint is actually added.
    pub fn next_constraint_id(&self) -> ConstraintID {
        let max_id = self
            .constraints()
            .keys()
            .chain(self.removed_constraints().keys())
            .map(|id| id.into_inner())
            .max()
            .unwrap_or(0);

        ConstraintID::from(max_id + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        assign, coeff,
        constraint::{Constraint, ConstraintID},
        linear,
        polynomial_base::{Linear, LinearMonomial},
        DecisionVariable, Function, VariableID,
    };

    use maplit::btreemap;

    #[test]
    fn test_insert_constraint_success() {
        // Create a simple instance with two decision variables
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
        };

        let objective = linear!(1) + coeff!(1.0);

        let mut instance = Instance::new(
            Sense::Minimize,
            objective.into(),
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();

        // Insert a new constraint using variable 1
        let constraint =
            Constraint::equal_to_zero(ConstraintID::from(10), (linear!(1) + coeff!(2.0)).into());
        let result = instance.insert_constraint(constraint.clone()).unwrap();

        // Should return None since no constraint with ID 10 existed before
        assert!(result.is_none());
        assert_eq!(instance.constraints.len(), 1);
        assert_eq!(
            instance.constraints.get(&ConstraintID::from(10)),
            Some(&constraint)
        );
    }

    #[test]
    fn test_insert_constraint_replace_existing() {
        // Create instance with one constraint
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(
            VariableID::from(1),
            DecisionVariable::binary(VariableID::from(1)),
        );
        decision_variables.insert(
            VariableID::from(2),
            DecisionVariable::binary(VariableID::from(2)),
        );

        let objective = Function::Linear(Linear::single_term(
            LinearMonomial::Variable(VariableID::from(1)),
            coeff!(1.0),
        ));

        let mut constraints = BTreeMap::new();
        let original_constraint =
            Constraint::equal_to_zero(ConstraintID::from(5), (linear!(1) + coeff!(1.0)).into());
        constraints.insert(ConstraintID::from(5), original_constraint.clone());

        let mut instance =
            Instance::new(Sense::Minimize, objective, decision_variables, constraints).unwrap();

        // Insert a new constraint with the same ID but using variable 2
        let new_constraint =
            Constraint::equal_to_zero(ConstraintID::from(5), (linear!(2) + coeff!(1.0)).into());
        let result = instance.insert_constraint(new_constraint.clone()).unwrap();

        // Should return the old constraint that was replaced
        assert_eq!(result, Some(original_constraint));
        assert_eq!(instance.constraints.len(), 1);
        assert_eq!(
            instance.constraints.get(&ConstraintID::from(5)),
            Some(&new_constraint)
        );
    }

    #[test]
    fn test_insert_constraint_undefined_variable() {
        // Create instance with only variable 1 and 2
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(
            VariableID::from(1),
            DecisionVariable::binary(VariableID::from(1)),
        );
        decision_variables.insert(
            VariableID::from(2),
            DecisionVariable::binary(VariableID::from(2)),
        );

        let objective = Function::Linear(Linear::single_term(
            LinearMonomial::Variable(VariableID::from(1)),
            coeff!(1.0),
        ));

        let mut instance = Instance::new(
            Sense::Minimize,
            objective,
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();

        // Try to insert constraint using undefined variable 999
        let constraint =
            Constraint::equal_to_zero(ConstraintID::from(10), (linear!(999) + coeff!(1.0)).into());
        let result = instance.insert_constraint(constraint);

        // Should fail with undefined variable error
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(
            err.to_string(),
            "Undefined variable ID is used: VariableID(999)"
        );
        // Ensure no constraint was added
        assert_eq!(instance.constraints.len(), 0);
    }

    #[test]
    fn test_insert_constraint_multiple_operations() {
        // Test multiple insertions and replacements
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(
            VariableID::from(1),
            DecisionVariable::binary(VariableID::from(1)),
        );
        decision_variables.insert(
            VariableID::from(2),
            DecisionVariable::binary(VariableID::from(2)),
        );
        decision_variables.insert(
            VariableID::from(3),
            DecisionVariable::binary(VariableID::from(3)),
        );

        let objective = Function::Linear(Linear::single_term(
            LinearMonomial::Variable(VariableID::from(1)),
            coeff!(1.0),
        ));

        let mut instance = Instance::new(
            Sense::Minimize,
            objective,
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();

        // Insert multiple constraints
        let constraint1 =
            Constraint::equal_to_zero(ConstraintID::from(1), (linear!(1) + coeff!(1.0)).into());
        let constraint2 =
            Constraint::equal_to_zero(ConstraintID::from(2), (linear!(2) + coeff!(1.0)).into());
        let constraint3 =
            Constraint::equal_to_zero(ConstraintID::from(3), (linear!(3) + coeff!(1.0)).into());

        assert!(instance
            .insert_constraint(constraint1.clone())
            .unwrap()
            .is_none());
        assert!(instance
            .insert_constraint(constraint2.clone())
            .unwrap()
            .is_none());
        assert!(instance
            .insert_constraint(constraint3.clone())
            .unwrap()
            .is_none());
        assert_eq!(instance.constraints.len(), 3);

        // Replace constraint 2 with new one
        let new_constraint2 =
            Constraint::equal_to_zero(ConstraintID::from(2), (linear!(1) + coeff!(1.0)).into());
        let replaced = instance.insert_constraint(new_constraint2.clone()).unwrap();
        assert_eq!(replaced, Some(constraint2));
        assert_eq!(instance.constraints.len(), 3);
        assert_eq!(
            instance.constraints.get(&ConstraintID::from(2)),
            Some(&new_constraint2)
        );
    }

    #[test]
    fn test_insert_constraint_with_dependency_key() {
        // Create instance with decision variables and dependency
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
            VariableID::from(3) => DecisionVariable::binary(VariableID::from(3)),
        };
        let objective = linear!(1) + coeff!(1.0);
        let mut instance = Instance::new(
            Sense::Minimize,
            objective.into(),
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();

        // Add a dependency: x2 = x1 + 1
        instance.decision_variable_dependency = assign! {
            2 <- linear!(1) + coeff!(1.0)
        };

        // Try to insert constraint using variable 2 (which is in dependency keys)
        let constraint =
            Constraint::equal_to_zero(ConstraintID::from(10), (linear!(2) + coeff!(1.0)).into());
        let result = instance.insert_constraint(constraint);
        assert_eq!(
            result.unwrap_err().to_string(),
            "Dependent variable cannot be used in objectives or constraints: VariableID(2)"
        );
        // Ensure no constraint was added
        assert_eq!(instance.constraints.len(), 0);
    }

    #[test]
    fn test_set_objective_with_dependency_key() {
        // Create instance with decision variables and dependency
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
        };
        let objective = linear!(1) + coeff!(1.0);
        let mut instance = Instance::new(
            Sense::Minimize,
            objective.into(),
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();

        // Add a dependency: x2 = x1 + 1
        instance.decision_variable_dependency = assign! {
            2 <- linear!(1) + coeff!(1.0)
        };

        // Try to set objective using variable 2 (which is in dependency keys)
        let new_objective = linear!(2) + coeff!(1.0);
        let result = instance.set_objective(new_objective.into());

        // Should fail with DependentVariableUsed error
        assert_eq!(
            result.unwrap_err().to_string(),
            "Dependent variable cannot be used in objectives or constraints: VariableID(2)"
        );
        // Ensure objective was not changed
        assert_eq!(instance.objective, Function::from(linear!(1) + coeff!(1.0)));
    }

    #[test]
    fn test_insert_constraint_replace_removed_constraint() {
        // Create instance with one active constraint and one removed constraint
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
        };

        let objective = (linear!(1) + coeff!(1.0)).into();
        let constraints = btreemap! {
            ConstraintID::from(1) => Constraint::equal_to_zero(
                ConstraintID::from(1),
                (linear!(1) + coeff!(1.0)).into(),
            ),
            ConstraintID::from(2) => Constraint::equal_to_zero(
                ConstraintID::from(2),
                (linear!(2) + coeff!(2.0)).into(),
            ),
        };

        let mut instance =
            Instance::new(Sense::Minimize, objective, decision_variables, constraints).unwrap();
        instance
            .relax_constraint(ConstraintID::from(2), "test".to_string(), [])
            .unwrap();

        // Verify initial state
        assert_eq!(instance.constraints.len(), 1);
        assert_eq!(instance.removed_constraints.len(), 1);

        // Insert a new constraint with the same ID as the removed constraint
        let new_constraint = Constraint::equal_to_zero(
            ConstraintID::from(2),
            (linear!(1) + linear!(2) + coeff!(3.0)).into(),
        );
        let result = instance.insert_constraint(new_constraint.clone()).unwrap();

        // Should return the old removed constraint
        assert_eq!(
            result,
            Some(Constraint::equal_to_zero(
                ConstraintID::from(2),
                (linear!(2) + coeff!(2.0)).into(),
            ))
        );

        assert_eq!(instance.constraints.len(), 1);
        assert_eq!(instance.removed_constraints.len(), 1);
        assert_eq!(
            instance
                .removed_constraints
                .get(&ConstraintID::from(2))
                .unwrap()
                .constraint,
            new_constraint
        );
    }

    #[test]
    fn test_next_constraint_id() {
        // Test basic case: empty instance
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
        };
        let objective = (linear!(1) + coeff!(1.0)).into();
        let instance = Instance::new(
            Sense::Minimize,
            objective,
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();
        assert_eq!(instance.next_constraint_id(), ConstraintID::from(1));

        // Test considering both active and removed constraints
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
        };
        let objective = (linear!(1) + coeff!(1.0)).into();
        let constraints = btreemap! {
            ConstraintID::from(3) => Constraint::equal_to_zero(
                ConstraintID::from(3),
                (linear!(1) + coeff!(1.0)).into(),
            ),
            ConstraintID::from(15) => Constraint::equal_to_zero(
                ConstraintID::from(15),
                (linear!(1) + coeff!(2.0)).into(),
            ),
        };
        let mut instance =
            Instance::new(Sense::Minimize, objective, decision_variables, constraints).unwrap();
        instance
            .relax_constraint(ConstraintID::from(15), "test".to_string(), [])
            .unwrap();

        // Should return 16 (max(3, 15) + 1)
        assert_eq!(instance.next_constraint_id(), ConstraintID::from(16));
    }
}
