use super::*;
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
            .constraints
            .remove(&id)
            .ok_or_else(|| anyhow!("Constraint with ID {:?} not found", id))?;
        self.removed_constraints.insert(
            id,
            RemovedConstraint {
                constraint: c,
                removed_reason,
                removed_reason_parameters: parameters.into_iter().collect(),
            },
        );
        Ok(())
    }

    pub fn restore_constraint(&mut self, id: ConstraintID) -> Result<()> {
        let rc = self
            .removed_constraints
            .remove(&id)
            .ok_or_else(|| anyhow!("Removed constraint with ID {:?} not found", id))?;

        let mut constraint = rc.constraint;

        // 1. Substitute fixed variables (those with substituted_value set)
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

        // 2. Substitute dependent variables
        if !self.decision_variable_dependency.is_empty() {
            crate::substitute_acyclic(
                &mut constraint.function,
                &self.decision_variable_dependency,
            )?;
        }

        self.constraints.insert(id, constraint);
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
            function: constraint_function,
            equality: Equality::LessThanOrEqualToZero,
            name: None,
            subscripts: Vec::new(),
            parameters: Default::default(),
            description: None,
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
        assert!(instance.constraints.is_empty());
        assert_eq!(instance.removed_constraints.len(), 1);

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
        assert_eq!(instance.constraints.len(), 1);
        assert!(instance.removed_constraints.is_empty());

        // Check the restored constraint has x1 substituted
        // Original: x1 + x2 - 10
        // After substituting x1=3: 3 + x2 - 10 = x2 - 7
        let restored_constraint = instance.constraints.get(&ConstraintID::from(1)).unwrap();
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
            function: constraint_function,
            equality: Equality::LessThanOrEqualToZero,
            name: None,
            subscripts: Vec::new(),
            parameters: Default::default(),
            description: None,
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
        assert!(instance.constraints.is_empty());
        assert_eq!(instance.removed_constraints.len(), 1);

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
        assert_eq!(instance.constraints.len(), 1);
        assert!(instance.removed_constraints.is_empty());

        // Check the restored constraint has x3 substituted with x1 + x2
        // Original: x3 - 10
        // After substituting x3 = x1 + x2: x1 + x2 - 10
        let restored_constraint = instance.constraints.get(&ConstraintID::from(1)).unwrap();
        let required_ids = restored_constraint.required_ids();

        // x3 should NOT be in the required IDs (it's been substituted)
        assert!(!required_ids.contains(&VariableID::from(3)));
        // x1 and x2 should be in the required IDs
        assert!(required_ids.contains(&VariableID::from(1)));
        assert!(required_ids.contains(&VariableID::from(2)));
    }
}
