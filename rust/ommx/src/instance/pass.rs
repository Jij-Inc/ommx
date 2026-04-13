use super::*;
use crate::constraint::RemovedData;
use crate::ATol;
use anyhow::{anyhow, Result};

impl Instance {
    pub fn relax_constraint(
        &mut self,
        id: ConstraintID,
        removed_reason: String,
        parameters: impl IntoIterator<Item = (String, String)>,
    ) -> Result<()> {
        let c = self
            .constraint_collection
            .active_mut()
            .remove(&id)
            .ok_or_else(|| anyhow!("Constraint with ID {:?} not found", id))?;
        self.constraint_collection.removed_mut().insert(
            id,
            Constraint {
                id: c.id,
                equality: c.equality,
                metadata: c.metadata,
                stage: RemovedData {
                    function: c.stage.function,
                    removed_reason,
                    removed_reason_parameters: parameters.into_iter().collect(),
                },
            },
        );

        // Invalidate constraint hints that reference the removed constraint
        self.constraint_hints
            .one_hot_constraints
            .retain(|hint| hint.id != id);
        self.constraint_hints.sos1_constraints.retain(|hint| {
            hint.binary_constraint_id != id && !hint.big_m_constraint_ids.contains(&id)
        });

        Ok(())
    }

    pub fn restore_constraint(&mut self, id: ConstraintID) -> Result<()> {
        let rc = self
            .constraint_collection
            .removed()
            .get(&id)
            .ok_or_else(|| anyhow!("Removed constraint with ID {:?} not found", id))?;

        // Clone the constraint first to avoid data loss if transformations fail
        let mut constraint: Constraint<crate::constraint::Created> = Constraint {
            id: rc.id,
            equality: rc.equality,
            metadata: rc.metadata.clone(),
            stage: crate::constraint::CreatedData {
                function: rc.stage.function.clone(),
            },
        };

        // 1. Substitute dependent variables first
        //    Dependency expansion may introduce fixed variables (e.g., x3 = x1 + x2 where x1 is fixed),
        //    so this must happen before partial_evaluate.
        if !self.decision_variable_dependency.is_empty() {
            crate::substitute_acyclic(
                &mut constraint.stage.function,
                &self.decision_variable_dependency,
            )?;
        }

        // 2. Substitute fixed variables (those with substituted_value set)
        //    This comes after dependency substitution to handle variables introduced by expansion.
        let fixed_state: v1::State = v1::State {
            entries: self
                .decision_variables
                .iter()
                .filter_map(|(id, dv)| dv.substituted_value().map(|v| (id.into_inner(), v)))
                .collect(),
        };
        if !fixed_state.entries.is_empty() {
            constraint.partial_evaluate(&fixed_state, ATol::default())?;
        }

        // Only remove from removed_constraints after all transformations succeed
        self.constraint_collection.removed_mut().remove(&id);
        self.constraint_collection
            .active_mut()
            .insert(id, constraint);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, constraint::Equality, linear, DecisionVariable, Sense, Substitute};
    use std::collections::BTreeMap;

    /// Test that restore_constraint correctly substitutes fixed variables.
    ///
    /// Scenario:
    /// 1. Create an Instance with variables x1 and x2
    /// 2. Add a constraint using x1: x1 + x2 <= 10
    /// 3. Relax the constraint
    /// 4. Set x1's substituted_value to 3.0
    /// 5. Restore the constraint
    /// 6. Verify the restored constraint has x1 substituted: x2 + 3 <= 10 (i.e., x2 - 7 <= 0)
    #[test]
    fn test_restore_constraint_with_fixed_variable() {
        // Create decision variables
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(
            VariableID::from(1),
            DecisionVariable::continuous(VariableID::from(1)),
        );
        decision_variables.insert(
            VariableID::from(2),
            DecisionVariable::continuous(VariableID::from(2)),
        );

        // Create constraint: x1 + x2 - 10 <= 0
        let constraint_function = Function::from(linear!(1) + linear!(2) + coeff!(-10.0));
        let mut constraints = BTreeMap::new();
        let constraint = Constraint {
            id: ConstraintID::from(1),
            equality: Equality::LessThanOrEqualToZero,
            metadata: crate::constraint::ConstraintMetadata::default(),
            stage: crate::constraint::CreatedData {
                function: constraint_function,
            },
        };
        constraints.insert(ConstraintID::from(1), constraint);

        // Create instance
        let objective = Function::from(linear!(1) + linear!(2));
        let mut instance =
            Instance::new(Sense::Minimize, objective, decision_variables, constraints).unwrap();

        // Relax the constraint
        instance
            .relax_constraint(ConstraintID::from(1), "test".to_string(), [])
            .unwrap();

        // Verify constraint is removed
        assert!(instance.constraints().is_empty());
        assert_eq!(instance.removed_constraints().len(), 1);

        // Fix x1 to 3.0 using partial_evaluate on the instance
        let fix_state = v1::State {
            entries: [(1, 3.0)].into_iter().collect(),
        };
        instance
            .partial_evaluate(&fix_state, ATol::default())
            .unwrap();

        // Verify x1 has substituted_value set
        assert_eq!(
            instance
                .decision_variables
                .get(&VariableID::from(1))
                .unwrap()
                .substituted_value(),
            Some(3.0)
        );

        // Restore the constraint
        instance.restore_constraint(ConstraintID::from(1)).unwrap();

        // Verify constraint is restored
        assert_eq!(instance.constraints().len(), 1);
        assert!(instance.removed_constraints().is_empty());

        // Check the restored constraint has x1 substituted
        // Original: x1 + x2 - 10
        // After substituting x1=3: 3 + x2 - 10 = x2 - 7
        let restored_constraint = instance.constraints().get(&ConstraintID::from(1)).unwrap();
        let required_ids = restored_constraint.required_ids();

        // x1 should NOT be in the required IDs (it's been substituted)
        assert!(!required_ids.contains(&VariableID::from(1)));
        // x2 should still be in the required IDs
        assert!(required_ids.contains(&VariableID::from(2)));
    }

    /// Test that restore_constraint correctly substitutes dependent variables.
    ///
    /// Scenario:
    /// 1. Create an Instance with variables x1, x2, x3
    /// 2. Add a constraint using x3: x3 <= 10
    /// 3. Relax the constraint
    /// 4. Add dependency x3 = x1 + x2
    /// 5. Restore the constraint
    /// 6. Verify the restored constraint has x3 substituted: x1 + x2 <= 10
    #[test]
    fn test_restore_constraint_with_dependent_variable() {
        // Create decision variables
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(
            VariableID::from(1),
            DecisionVariable::continuous(VariableID::from(1)),
        );
        decision_variables.insert(
            VariableID::from(2),
            DecisionVariable::continuous(VariableID::from(2)),
        );
        decision_variables.insert(
            VariableID::from(3),
            DecisionVariable::continuous(VariableID::from(3)),
        );

        // Create constraint: x3 - 10 <= 0
        let constraint_function = Function::from(linear!(3) + coeff!(-10.0));
        let mut constraints = BTreeMap::new();
        let constraint = Constraint {
            id: ConstraintID::from(1),
            equality: Equality::LessThanOrEqualToZero,
            metadata: crate::constraint::ConstraintMetadata::default(),
            stage: crate::constraint::CreatedData {
                function: constraint_function,
            },
        };
        constraints.insert(ConstraintID::from(1), constraint);

        // Create instance
        let objective = Function::from(linear!(1) + linear!(2) + linear!(3));
        let mut instance =
            Instance::new(Sense::Minimize, objective, decision_variables, constraints).unwrap();

        // Relax the constraint
        instance
            .relax_constraint(ConstraintID::from(1), "test".to_string(), [])
            .unwrap();

        // Verify constraint is removed
        assert!(instance.constraints().is_empty());
        assert_eq!(instance.removed_constraints().len(), 1);

        // Add dependency x3 = x1 + x2 using substitute_one on the instance
        // This will add the dependency to decision_variable_dependency
        let substitution = Function::from(linear!(1) + linear!(2));
        instance = instance
            .substitute_one(VariableID::from(3), &substitution)
            .unwrap();

        // Verify dependency is set
        assert_eq!(instance.decision_variable_dependency.len(), 1);
        assert!(instance
            .decision_variable_dependency
            .get(&VariableID::from(3))
            .is_some());

        // Restore the constraint
        instance.restore_constraint(ConstraintID::from(1)).unwrap();

        // Verify constraint is restored
        assert_eq!(instance.constraints().len(), 1);
        assert!(instance.removed_constraints().is_empty());

        // Check the restored constraint has x3 substituted with x1 + x2
        // Original: x3 - 10
        // After substituting x3 = x1 + x2: x1 + x2 - 10
        let restored_constraint = instance.constraints().get(&ConstraintID::from(1)).unwrap();
        let required_ids = restored_constraint.required_ids();

        // x3 should NOT be in the required IDs (it's been substituted)
        assert!(!required_ids.contains(&VariableID::from(3)));
        // x1 and x2 should be in the required IDs
        assert!(required_ids.contains(&VariableID::from(1)));
        assert!(required_ids.contains(&VariableID::from(2)));
    }

    /// Test that restore_constraint correctly handles the case where
    /// dependency expansion introduces fixed variables.
    ///
    /// Scenario:
    /// 1. Create an Instance with variables x1, x2, x3
    /// 2. Add a constraint using x3: x3 <= 10
    /// 3. Relax the constraint
    /// 4. Fix x1 to 3.0 (set substituted_value)
    /// 5. Add dependency x3 = x1 + x2
    /// 6. Restore the constraint
    /// 7. Expected: constraint should have both x3 and x1 substituted
    ///    Original: x3 - 10
    ///    After x3 = x1 + x2: x1 + x2 - 10
    ///    After x1 = 3: x2 + 3 - 10 = x2 - 7
    #[test]
    fn test_restore_constraint_with_fixed_variable_in_dependency() {
        // Create decision variables
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(
            VariableID::from(1),
            DecisionVariable::continuous(VariableID::from(1)),
        );
        decision_variables.insert(
            VariableID::from(2),
            DecisionVariable::continuous(VariableID::from(2)),
        );
        decision_variables.insert(
            VariableID::from(3),
            DecisionVariable::continuous(VariableID::from(3)),
        );

        // Create constraint: x3 - 10 <= 0
        let constraint_function = Function::from(linear!(3) + coeff!(-10.0));
        let mut constraints = BTreeMap::new();
        let constraint = Constraint {
            id: ConstraintID::from(1),
            equality: Equality::LessThanOrEqualToZero,
            metadata: crate::constraint::ConstraintMetadata::default(),
            stage: crate::constraint::CreatedData {
                function: constraint_function,
            },
        };
        constraints.insert(ConstraintID::from(1), constraint);
        // Create instance
        let objective = Function::from(linear!(1) + linear!(2) + linear!(3));
        let mut instance =
            Instance::new(Sense::Minimize, objective, decision_variables, constraints).unwrap();

        // Relax the constraint
        instance
            .relax_constraint(ConstraintID::from(1), "test".to_string(), [])
            .unwrap();

        // Fix x1 to 3.0 BEFORE adding the dependency
        let fix_state = v1::State {
            entries: [(1, 3.0)].into_iter().collect(),
        };
        instance
            .partial_evaluate(&fix_state, ATol::default())
            .unwrap();

        // Add dependency x3 = x1 + x2 AFTER fixing x1
        // This means when we expand x3, we get x1 + x2, and x1 should also be substituted
        let substitution = Function::from(linear!(1) + linear!(2));
        instance = instance
            .substitute_one(VariableID::from(3), &substitution)
            .unwrap();

        // Restore the constraint
        instance.restore_constraint(ConstraintID::from(1)).unwrap();

        // Check the restored constraint has both x3 and x1 substituted
        // Original: x3 - 10
        // After x3 = x1 + x2: x1 + x2 - 10
        // After x1 = 3: x2 - 7
        let restored_constraint = instance.constraints().get(&ConstraintID::from(1)).unwrap();
        let required_ids = restored_constraint.required_ids();

        // x3 should NOT be in the required IDs (it's been substituted)
        assert!(!required_ids.contains(&VariableID::from(3)));
        // x1 should NOT be in the required IDs (it's been substituted via fixed value)
        assert!(
            !required_ids.contains(&VariableID::from(1)),
            "x1 should be substituted because it was fixed before dependency was added"
        );
        // x2 should still be in the required IDs
        assert!(required_ids.contains(&VariableID::from(2)));
    }
}
