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

        // Check that no one-hot or SOS1 variable is being substituted.
        for oh in self.one_hot_constraint_collection.active().values() {
            for var_id in &oh.variables {
                if substituted_variables.contains(var_id) {
                    return Err(SubstitutionError::OneHotVariableSubstitution {
                        variable: *var_id,
                        constraint_id: oh.id,
                    });
                }
            }
        }
        for sos1 in self.sos1_constraint_collection.active().values() {
            for var_id in &sos1.variables {
                if substituted_variables.contains(var_id) {
                    return Err(SubstitutionError::Sos1VariableSubstitution {
                        variable: *var_id,
                        constraint_id: sos1.id,
                    });
                }
            }
        }

        // Apply substitution to the existing decision_variable_dependency
        substitute_acyclic(&mut self.decision_variable_dependency, acyclic)?;

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
    use crate::{coeff, constraint::Equality, linear, DecisionVariable, Sense};
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
