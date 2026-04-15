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

        // Check active constraints only.
        // Removed constraints are not checked here; they will be substituted
        // when restored via `restore_constraint`. Constraint hints for removed
        // constraints are discarded when the constraint is removed.
        for (constraint_id, constraint) in self.constraint_collection.active() {
            let required_ids = constraint.required_ids();
            if !required_ids.is_disjoint(&substituted_variables) {
                affected_constraint_ids.insert(*constraint_id);
            }
        }

        // Apply substitution to the objective function
        substitute_acyclic(&mut self.objective, acyclic)?;

        // Apply substitution only to affected active constraints.
        // Removed constraints are not substituted here; they will be substituted
        // when restored via `restore_constraint`.
        for constraint_id in &affected_constraint_ids {
            if let Some(constraint) = self
                .constraint_collection
                .active_mut()
                .get_mut(constraint_id)
            {
                substitute_acyclic(&mut constraint.stage.function, acyclic)?;
            }
        }

        // Check that no indicator constraint's indicator_variable is being substituted.
        // Substituting an indicator variable would change the constraint type
        // (e.g. fixing it to 1 makes it a regular constraint, fixing to 0 removes it).
        // This is not yet supported; fail explicitly rather than silently producing
        // an inconsistent result.
        for ic in self.indicator_constraint_collection.active().values() {
            if substituted_variables.contains(&ic.indicator_variable) {
                return Err(SubstitutionError::IndicatorVariableSubstitution {
                    indicator_variable: ic.indicator_variable,
                    constraint_id: ic.id,
                });
            }
        }

        // Apply substitution to the function part of active indicator constraints
        for ic in self
            .indicator_constraint_collection
            .active_mut()
            .values_mut()
        {
            let required_ids = ic.stage.function.required_ids();
            if !required_ids.is_disjoint(&substituted_variables) {
                substitute_acyclic(&mut ic.stage.function, acyclic)?;
            }
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
            .one_hot_constraints
            .retain(|hint| !affected_constraint_ids.contains(&hint.id));
        self.constraint_hints.sos1_constraints.retain(|hint| {
            !affected_constraint_ids.contains(&hint.binary_constraint_id)
                && hint
                    .big_m_constraint_ids
                    .is_disjoint(&affected_constraint_ids)
        });

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
            equality: Equality::LessThanOrEqualToZero,
            metadata: crate::constraint::ConstraintMetadata::default(),
            stage: crate::constraint::CreatedData {
                function: constraint_function,
            },
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
            equality: Equality::EqualToZero,
            metadata: crate::constraint::ConstraintMetadata::default(),
            stage: crate::constraint::CreatedData {
                function: constraint1_function,
            },
        };
        constraints.insert(ConstraintID::from(1), constraint1);

        // Create constraint that doesn't depend on x1: x2 + x3 == 1 (constraint_id=2)
        let constraint2_function = Function::from(linear!(2) + linear!(3) + coeff!(-1.0));
        let constraint2 = Constraint {
            id: ConstraintID::from(2),
            equality: Equality::EqualToZero,
            metadata: crate::constraint::ConstraintMetadata::default(),
            stage: crate::constraint::CreatedData {
                function: constraint2_function,
            },
        };
        constraints.insert(ConstraintID::from(2), constraint2);

        // Create constraint hints
        let one_hot_for_constraint1 = OneHot {
            id: ConstraintID::from(1),
            variables: [VariableID::from(1), VariableID::from(2)]
                .into_iter()
                .collect(),
        };
        let one_hot_for_constraint2 = OneHot {
            id: ConstraintID::from(2),
            variables: [VariableID::from(2), VariableID::from(3)]
                .into_iter()
                .collect(),
        };
        let sos1 = Sos1 {
            binary_constraint_id: ConstraintID::from(1),
            big_m_constraint_ids: [ConstraintID::from(2)].into_iter().collect(),
            variables: [
                VariableID::from(1),
                VariableID::from(2),
                VariableID::from(3),
            ]
            .into_iter()
            .collect(),
        };

        let constraint_hints = ConstraintHints {
            one_hot_constraints: vec![one_hot_for_constraint1, one_hot_for_constraint2],
            sos1_constraints: vec![sos1],
        };

        // Create objective
        let objective = Function::from(linear!(1) + linear!(2) + linear!(3));

        let instance = Instance::new(Sense::Minimize, objective, decision_variables, constraints)
            .unwrap()
            .with_constraint_hints(constraint_hints)
            .unwrap();

        // Before substitution, verify we have 2 OneHot constraints and 1 SOS1 constraint
        assert_eq!(instance.constraint_hints.one_hot_constraints.len(), 2);
        assert_eq!(instance.constraint_hints.sos1_constraints.len(), 1);

        // Substitute x1 with a constant: x1 = 1
        let substitution = Function::from(coeff!(1.0));
        let result = instance
            .substitute_one(VariableID::from(1), &substitution)
            .unwrap();

        // After substitution:
        // - OneHot for constraint1 should be removed (constraint1 depends on x1)
        // - OneHot for constraint2 should remain (constraint2 doesn't depend on x1)
        // - SOS1 should be removed (it references constraint1 which depends on x1)
        assert_eq!(result.constraint_hints.one_hot_constraints.len(), 1);
        assert_eq!(
            result.constraint_hints.one_hot_constraints[0].id,
            ConstraintID::from(2)
        );
        assert_eq!(result.constraint_hints.sos1_constraints.len(), 0);
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
            equality: Equality::EqualToZero,
            metadata: crate::constraint::ConstraintMetadata::default(),
            stage: crate::constraint::CreatedData {
                function: constraint1_function,
            },
        };
        constraints.insert(ConstraintID::from(1), constraint1);

        // Create removed constraint that depends on x1: x1 + x3 == 1 (constraint_id=2)
        let mut removed_constraints = BTreeMap::new();
        let removed_constraint_function = Function::from(linear!(1) + linear!(3) + coeff!(-1.0));
        let removed_constraint = RemovedConstraint {
            id: ConstraintID::from(2),
            equality: Equality::EqualToZero,
            metadata: crate::constraint::ConstraintMetadata::default(),
            stage: crate::constraint::RemovedData {
                function: removed_constraint_function,
                removed_reason: crate::constraint::RemovedReason {
                    reason: "test".to_string(),
                    parameters: Default::default(),
                },
            },
        };
        removed_constraints.insert(ConstraintID::from(2), removed_constraint);

        // Create constraint hints - OneHot for removed constraint should also be removed
        let one_hot_for_active = OneHot {
            id: ConstraintID::from(1),
            variables: [VariableID::from(1), VariableID::from(2)]
                .into_iter()
                .collect(),
        };
        let one_hot_for_removed = OneHot {
            id: ConstraintID::from(2),
            variables: [VariableID::from(1), VariableID::from(3)]
                .into_iter()
                .collect(),
        };

        let constraint_hints = ConstraintHints {
            one_hot_constraints: vec![one_hot_for_active, one_hot_for_removed],
            sos1_constraints: vec![],
        };

        // Create objective
        let objective = Function::from(linear!(1) + linear!(2) + linear!(3));

        // Create instance with both active and removed constraints
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(objective)
            .decision_variables(decision_variables)
            .constraints(constraints)
            .removed_constraints(removed_constraints)
            .build()
            .unwrap();
        instance.constraint_hints = constraint_hints;

        // Before substitution, verify we have 2 OneHot constraints
        assert_eq!(instance.constraint_hints.one_hot_constraints.len(), 2);

        // Substitute x1 with a constant: x1 = 1
        let substitution = Function::from(coeff!(1.0));
        let result = instance
            .substitute_one(VariableID::from(1), &substitution)
            .unwrap();

        // After substitution:
        // Only the OneHot constraint for the active constraint is removed.
        // The hint for the removed constraint remains (it should have been
        // discarded when the constraint was removed, not during substitution).
        assert_eq!(result.constraint_hints.one_hot_constraints.len(), 1);
        assert_eq!(
            result.constraint_hints.one_hot_constraints[0].id,
            ConstraintID::from(2)
        );
    }

    #[test]
    fn test_substitute_indicator_function() {
        // Substituting a variable in the indicator's function should work
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
            VariableID::from(10),
            DecisionVariable::binary(VariableID::from(10)),
        );

        let objective = Function::from(linear!(1));

        let mut indicator_constraints = BTreeMap::new();
        indicator_constraints.insert(
            crate::IndicatorConstraintID::from(1),
            crate::IndicatorConstraint::new(
                crate::IndicatorConstraintID::from(1),
                VariableID::from(10),
                Equality::LessThanOrEqualToZero,
                Function::from(linear!(1) + coeff!(-5.0)),
            ),
        );

        let instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(objective)
            .decision_variables(decision_variables)
            .constraints(BTreeMap::new())
            .indicator_constraints(indicator_constraints)
            .build()
            .unwrap();

        // Substitute x1 = x2 + 1
        let assignments = crate::AcyclicAssignments::new(vec![(
            VariableID::from(1),
            Function::from(linear!(2) + coeff!(1.0)),
        )])
        .unwrap();

        let result = instance.substitute_acyclic(&assignments).unwrap();

        // Indicator constraint should still exist with substituted function
        assert_eq!(result.indicator_constraints().len(), 1);
    }

    #[test]
    fn test_substitute_indicator_variable_fails() {
        // Substituting the indicator variable itself should fail
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(
            VariableID::from(1),
            DecisionVariable::continuous(VariableID::from(1)),
        );
        decision_variables.insert(
            VariableID::from(10),
            DecisionVariable::binary(VariableID::from(10)),
        );

        let objective = Function::from(linear!(1));

        let mut indicator_constraints = BTreeMap::new();
        indicator_constraints.insert(
            crate::IndicatorConstraintID::from(1),
            crate::IndicatorConstraint::new(
                crate::IndicatorConstraintID::from(1),
                VariableID::from(10),
                Equality::LessThanOrEqualToZero,
                Function::from(linear!(1) + coeff!(-5.0)),
            ),
        );

        let instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(objective)
            .decision_variables(decision_variables)
            .constraints(BTreeMap::new())
            .indicator_constraints(indicator_constraints)
            .build()
            .unwrap();

        // Try to substitute the indicator variable x10
        let assignments = crate::AcyclicAssignments::new(vec![(
            VariableID::from(10),
            Function::from(coeff!(1.0)),
        )])
        .unwrap();

        let result = instance.substitute_acyclic(&assignments);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SubstitutionError::IndicatorVariableSubstitution {
                indicator_variable,
                constraint_id,
            } if indicator_variable == VariableID::from(10)
                && constraint_id == crate::IndicatorConstraintID::from(1)
        ));
    }
}
