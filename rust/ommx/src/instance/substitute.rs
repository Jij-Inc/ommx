use super::*;
use crate::{
    substitute_acyclic, substitute_one_via_acyclic, Function, Substitute, SubstitutionError,
    VariableID,
};

impl Substitute for Instance {
    type Output = Self;

    fn substitute_acyclic(
        mut self,
        acyclic: &crate::AcyclicAssignments,
    ) -> Result<Self::Output, crate::SubstitutionError> {
        // Get the set of variables being substituted
        let substituted_variables: std::collections::BTreeSet<VariableID> =
            acyclic.iter().map(|(var_id, _)| *var_id).collect();

        // Identify constraint IDs that depend on substituted variables
        let mut affected_constraint_ids = std::collections::BTreeSet::new();

        // Check active constraints
        for (constraint_id, constraint) in &self.constraints {
            let required_ids = constraint.required_ids();
            if !required_ids.is_disjoint(&substituted_variables) {
                affected_constraint_ids.insert(*constraint_id);
            }
        }

        // Check removed constraints
        for (constraint_id, removed_constraint) in &self.removed_constraints {
            let required_ids = removed_constraint.constraint.required_ids();
            if !required_ids.is_disjoint(&substituted_variables) {
                affected_constraint_ids.insert(*constraint_id);
            }
        }

        // Apply substitution to the objective function
        substitute_acyclic(&mut self.objective, acyclic)?;

        // Apply substitution to all constraints
        for constraint in self.constraints.values_mut() {
            substitute_acyclic(&mut constraint.function, acyclic)?;
        }

        // Apply substitution to all removed constraints
        for removed_constraint in self.removed_constraints.values_mut() {
            substitute_acyclic(&mut removed_constraint.constraint.function, acyclic)?;
        }

        // Apply substitution to the existing decision_variable_dependency
        substitute_acyclic(&mut self.decision_variable_dependency, acyclic)?;

        // Remove constraint hints that reference affected constraints
        //
        // FIXME
        // ------
        // - Currently, we remove all affected constraint hints, but some can still be valid.
        //   - e.g. an one-hot constraint x1 + x2 = 1 is still one-hot after substituting x1 = x3 + x4.
        self.constraint_hints
            .remove_hints_for_constraints(&affected_constraint_ids);

        Ok(self)
    }

    fn substitute_one(
        self,
        assigned: VariableID,
        f: &Function,
    ) -> Result<Self::Output, SubstitutionError> {
        substitute_one_via_acyclic(self, assigned, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coeff,
        constraint::Equality,
        constraint_hints::{OneHot, Sos1},
        linear, DecisionVariable, Sense,
    };
    use std::collections::BTreeMap;

    #[test]
    fn test_instance_substitute() {
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

        // Create a simple instance: minimize x1 + 2*x2, subject to x1 + x2 <= 10
        let objective = Function::from(linear!(1) + coeff!(2.0) * linear!(2));
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
        let _constraint_hints = ConstraintHints::default();

        let instance =
            Instance::new(Sense::Minimize, objective, decision_variables, constraints).unwrap();

        // Substitute x1 with x3 + 1
        let substitution = Function::from(linear!(3) + coeff!(1.0));
        let result = instance
            .substitute_one(VariableID::from(1), &substitution)
            .unwrap();

        // Check that the decision_variable_dependency contains the assignment x1 <- x3 + 1
        assert_eq!(result.decision_variable_dependency.len(), 1);
        assert!(result
            .decision_variable_dependency
            .get(&VariableID::from(1))
            .is_some());
    }

    #[test]
    fn test_constraint_hints_removal_on_substitute() {
        // Create decision variables
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

        // Create constraint that depends on x1: x1 + x2 == 1 (constraint_id=1)
        let mut constraints = BTreeMap::new();
        let constraint1_function = Function::from(linear!(1) + linear!(2) + coeff!(-1.0));
        let constraint1 = Constraint {
            id: ConstraintID::from(1),
            function: constraint1_function,
            equality: Equality::EqualToZero,
            name: None,
            subscripts: Vec::new(),
            parameters: Default::default(),
            description: None,
        };
        constraints.insert(ConstraintID::from(1), constraint1);

        // Create constraint that doesn't depend on x1: x2 + x3 == 1 (constraint_id=2)
        let constraint2_function = Function::from(linear!(2) + linear!(3) + coeff!(-1.0));
        let constraint2 = Constraint {
            id: ConstraintID::from(2),
            function: constraint2_function,
            equality: Equality::EqualToZero,
            name: None,
            subscripts: Vec::new(),
            parameters: Default::default(),
            description: None,
        };
        constraints.insert(ConstraintID::from(2), constraint2);

        // Create additional constraints for Sos1 big-M constraints
        let constraint3_function = Function::from(linear!(1) + coeff!(-1.0));
        let constraint3 = Constraint {
            id: ConstraintID::from(3),
            function: constraint3_function,
            equality: Equality::EqualToZero,
            name: None,
            subscripts: Vec::new(),
            parameters: Default::default(),
            description: None,
        };
        constraints.insert(ConstraintID::from(3), constraint3);

        let constraint4_function = Function::from(linear!(2) + coeff!(-1.0));
        let constraint4 = Constraint {
            id: ConstraintID::from(4),
            function: constraint4_function,
            equality: Equality::EqualToZero,
            name: None,
            subscripts: Vec::new(),
            parameters: Default::default(),
            description: None,
        };
        constraints.insert(ConstraintID::from(4), constraint4);

        // Create additional constraint for Sos1 big-M constraint for variable 3
        let constraint5_function = Function::from(linear!(3) + coeff!(-1.0));
        let constraint5 = Constraint {
            id: ConstraintID::from(5),
            function: constraint5_function,
            equality: Equality::EqualToZero,
            name: None,
            subscripts: Vec::new(),
            parameters: Default::default(),
            description: None,
        };
        constraints.insert(ConstraintID::from(5), constraint5);

        // Create constraint hints
        let one_hot_for_constraint1 = OneHot::new(
            ConstraintID::from(1),
            [VariableID::from(1), VariableID::from(2)]
                .into_iter()
                .collect(),
        );
        let one_hot_for_constraint2 = OneHot::new(
            ConstraintID::from(2),
            [VariableID::from(2), VariableID::from(3)]
                .into_iter()
                .collect(),
        );
        let sos1 = Sos1::new(
            ConstraintID::from(1),
            [
                (VariableID::from(1), Some(ConstraintID::from(3))),
                (VariableID::from(2), Some(ConstraintID::from(4))),
                (VariableID::from(3), Some(ConstraintID::from(5))), // Add missing big-M constraint for variable 3
            ]
            .into_iter()
            .collect(),
        );

        let constraint_hints = ConstraintHints::new(
            vec![one_hot_for_constraint1, one_hot_for_constraint2],
            vec![sos1],
        );

        // Create objective
        let objective = Function::from(linear!(1) + linear!(2) + linear!(3));

        let instance = Instance::new(Sense::Minimize, objective, decision_variables, constraints)
            .unwrap()
            .with_constraint_hints(constraint_hints)
            .unwrap();

        // Before substitution, verify we have 2 OneHot constraints and 1 SOS1 constraint
        assert_eq!(instance.constraint_hints.one_hot_constraints().len(), 2);
        assert_eq!(instance.constraint_hints.sos1_constraints().len(), 1);

        // Substitute x1 with a constant: x1 = 1
        let substitution = Function::from(coeff!(1.0));
        let result = instance
            .substitute_one(VariableID::from(1), &substitution)
            .unwrap();

        // After substitution:
        // - OneHot for constraint1 should be removed (constraint1 depends on x1)
        // - OneHot for constraint2 should remain (constraint2 doesn't depend on x1)
        // - SOS1 should be removed (it references constraint1 which depends on x1)
        assert_eq!(result.constraint_hints.one_hot_constraints().len(), 1);
        assert_eq!(
            result.constraint_hints.one_hot_constraints()[0].id(),
            &ConstraintID::from(2)
        );
        assert_eq!(result.constraint_hints.sos1_constraints().len(), 0);
    }

    #[test]
    fn test_constraint_hints_removal_with_removed_constraints() {
        // Create decision variables
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

        // Create active constraint: x1 + x2 == 1 (constraint_id=1)
        let mut constraints = BTreeMap::new();
        let constraint1_function = Function::from(linear!(1) + linear!(2) + coeff!(-1.0));
        let constraint1 = Constraint {
            id: ConstraintID::from(1),
            function: constraint1_function,
            equality: Equality::EqualToZero,
            name: None,
            subscripts: Vec::new(),
            parameters: Default::default(),
            description: None,
        };
        constraints.insert(ConstraintID::from(1), constraint1);

        // Create removed constraint that depends on x1: x1 + x3 == 1 (constraint_id=2)
        let mut removed_constraints = BTreeMap::new();
        let removed_constraint_function = Function::from(linear!(1) + linear!(3) + coeff!(-1.0));
        let removed_constraint_inner = Constraint {
            id: ConstraintID::from(2),
            function: removed_constraint_function,
            equality: Equality::EqualToZero,
            name: None,
            subscripts: Vec::new(),
            parameters: Default::default(),
            description: None,
        };
        let removed_constraint = RemovedConstraint {
            constraint: removed_constraint_inner,
            removed_reason: "test".to_string(),
            removed_reason_parameters: Default::default(),
        };
        removed_constraints.insert(ConstraintID::from(2), removed_constraint);

        // Create constraint hints - OneHot for removed constraint should also be removed
        let one_hot_for_active = OneHot::new(
            ConstraintID::from(1),
            [VariableID::from(1), VariableID::from(2)]
                .into_iter()
                .collect(),
        );
        let one_hot_for_removed = OneHot::new(
            ConstraintID::from(2),
            [VariableID::from(1), VariableID::from(3)]
                .into_iter()
                .collect(),
        );

        let constraint_hints =
            ConstraintHints::new(vec![one_hot_for_active, one_hot_for_removed], vec![]);

        // Create objective
        let objective = Function::from(linear!(1) + linear!(2) + linear!(3));

        // Create instance directly since new() only accepts active constraints
        let mut instance =
            Instance::new(Sense::Minimize, objective, decision_variables, constraints).unwrap();

        // Manually set removed constraints and constraint hints
        instance.removed_constraints = removed_constraints;
        instance.constraint_hints = constraint_hints;

        // Before substitution, verify we have 2 OneHot constraints
        assert_eq!(instance.constraint_hints.one_hot_constraints().len(), 2);

        // Substitute x1 with a constant: x1 = 1
        let substitution = Function::from(coeff!(1.0));
        let result = instance
            .substitute_one(VariableID::from(1), &substitution)
            .unwrap();

        // After substitution:
        // Both OneHot constraints should be removed because:
        // - constraint1 (active) depends on x1
        // - constraint2 (removed) also depends on x1
        assert_eq!(result.constraint_hints.one_hot_constraints().len(), 0);
    }
}
