use super::*;
use crate::ATol;
use anyhow::Result;

impl Instance {
    pub fn relax_constraint(
        &mut self,
        id: ConstraintID,
        removed_reason: String,
        parameters: impl IntoIterator<Item = (String, String)>,
    ) -> Result<()> {
        self.constraint_collection.relax(
            id,
            crate::constraint::RemovedReason {
                reason: removed_reason,
                parameters: parameters.into_iter().collect(),
            },
        )?;

        Ok(())
    }

    pub fn restore_constraint(&mut self, id: ConstraintID) -> Result<()> {
        self.constraint_collection.restore(id)?;
        let fixed_state = self.fixed_state();
        let constraint = self
            .constraint_collection
            .get_active_mut(&id)
            .expect("restore() just inserted this id into active");

        // The following operations are infallible for regular constraints:
        // - substitute_acyclic only fails on cyclic dependencies, which AcyclicAssignments prevents
        // - Constraint::partial_evaluate delegates to Function::partial_evaluate, which is infallible
        // No rollback is needed.
        if !self.decision_variable_dependency.is_empty() {
            crate::substitute_acyclic(
                &mut constraint.stage.function,
                &self.decision_variable_dependency,
            )?;
        }
        if !fixed_state.entries.is_empty() {
            constraint.partial_evaluate(&fixed_state, ATol::default())?;
        }
        Ok(())
    }

    pub fn relax_indicator_constraint(
        &mut self,
        id: crate::IndicatorConstraintID,
        removed_reason: String,
        parameters: impl IntoIterator<Item = (String, String)>,
    ) -> Result<()> {
        self.indicator_constraint_collection.relax(
            id,
            crate::constraint::RemovedReason {
                reason: removed_reason,
                parameters: parameters.into_iter().collect(),
            },
        )?;
        Ok(())
    }

    pub fn restore_indicator_constraint(&mut self, id: crate::IndicatorConstraintID) -> Result<()> {
        // Check before restoring: if dependency contains the indicator variable, reject
        let indicator_variable = self
            .indicator_constraint_collection
            .removed()
            .get(&id)
            .ok_or_else(
                || crate::error!({ ?id }, "Removed indicator constraint with ID {id:?} not found"),
            )?
            .0
            .indicator_variable;

        if self
            .decision_variable_dependency
            .get(&indicator_variable)
            .is_some()
        {
            crate::bail!(
                { ?id, ?indicator_variable },
                "Cannot restore indicator constraint {id:?}: indicator variable {indicator_variable:?} has been substituted",
            );
        }

        if self
            .fixed_decision_variable_values()
            .contains_key(&indicator_variable)
        {
            crate::bail!(
                { ?id, ?indicator_variable },
                "Cannot restore indicator constraint {id:?}: indicator variable {indicator_variable:?} has been fixed",
            );
        }

        self.indicator_constraint_collection.restore(id)?;
        let fixed_state = self.fixed_state();
        let ic = self
            .indicator_constraint_collection
            .get_active_mut(&id)
            .expect("restore() just inserted this id into active");

        // The following operations are infallible because:
        // - substitute_acyclic only fails on cyclic dependencies, which AcyclicAssignments prevents
        // - IndicatorConstraint::partial_evaluate fails only if the indicator variable is in
        //   fixed_state, but we already checked that above before restoring
        // No rollback is needed.
        if !self.decision_variable_dependency.is_empty() {
            crate::substitute_acyclic(&mut ic.stage.function, &self.decision_variable_dependency)?;
        }
        if !fixed_state.entries.is_empty() {
            ic.partial_evaluate(&fixed_state, ATol::default())?;
        }
        Ok(())
    }

    /// Build a State containing all table-owned fixed variable values.
    fn fixed_state(&self) -> v1::State {
        v1::State {
            entries: self
                .fixed_decision_variable_values()
                .iter()
                .map(|(id, value)| (id.into_inner(), *value))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, constraint::Equality, linear, DecisionVariable, Sense, Substitute};
    use std::collections::BTreeMap;

    #[test]
    fn test_restore_constraint_with_fixed_variable() {
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(VariableID::from(1), DecisionVariable::continuous());
        decision_variables.insert(VariableID::from(2), DecisionVariable::continuous());

        let constraint_function =
            Function::from(((linear!(1) + linear!(2)).unwrap() + coeff!(-10.0)).unwrap());
        let mut constraints = BTreeMap::new();
        let constraint = Constraint {
            equality: Equality::LessThanOrEqualToZero,
            stage: crate::constraint::CreatedData {
                function: constraint_function,
            },
        };
        constraints.insert(ConstraintID::from(1), constraint);

        let objective = Function::from(linear!(1) + linear!(2));
        let mut instance =
            Instance::new(Sense::Minimize, objective, decision_variables, constraints).unwrap();

        instance
            .relax_constraint(ConstraintID::from(1), "test".to_string(), [])
            .unwrap();
        assert!(instance.constraints().is_empty());
        assert_eq!(instance.removed_constraints().len(), 1);

        let fix_state = v1::State {
            entries: [(1, 3.0)].into_iter().collect(),
        };
        instance
            .partial_evaluate(&fix_state, ATol::default())
            .unwrap();

        instance.restore_constraint(ConstraintID::from(1)).unwrap();
        assert_eq!(instance.constraints().len(), 1);
        assert!(instance.removed_constraints().is_empty());

        // Original: x1 + x2 - 10, after x1=3: x2 - 7
        let restored = instance.constraints().get(&ConstraintID::from(1)).unwrap();
        assert!(!restored.required_ids().contains(&VariableID::from(1)));
        assert!(restored.required_ids().contains(&VariableID::from(2)));
    }

    #[test]
    fn test_restore_constraint_with_dependent_variable() {
        let mut decision_variables = BTreeMap::new();
        for i in 1..=3 {
            decision_variables.insert(VariableID::from(i), DecisionVariable::continuous());
        }

        let mut constraints = BTreeMap::new();
        constraints.insert(
            ConstraintID::from(1),
            Constraint {
                equality: Equality::LessThanOrEqualToZero,
                stage: crate::constraint::CreatedData {
                    function: Function::from(linear!(3) + coeff!(-10.0)),
                },
            },
        );

        let mut instance = Instance::new(
            Sense::Minimize,
            Function::from(((linear!(1) + linear!(2)).unwrap() + linear!(3)).unwrap()),
            decision_variables,
            constraints,
        )
        .unwrap();

        instance
            .relax_constraint(ConstraintID::from(1), "test".to_string(), [])
            .unwrap();

        let substitution = Function::from(linear!(1) + linear!(2));
        instance = instance
            .substitute_one(VariableID::from(3), &substitution)
            .unwrap();

        instance.restore_constraint(ConstraintID::from(1)).unwrap();

        // x3 substituted with x1 + x2
        let restored = instance.constraints().get(&ConstraintID::from(1)).unwrap();
        assert!(!restored.required_ids().contains(&VariableID::from(3)));
        assert!(restored.required_ids().contains(&VariableID::from(1)));
        assert!(restored.required_ids().contains(&VariableID::from(2)));
    }

    #[test]
    fn test_restore_constraint_with_fixed_variable_in_dependency() {
        let mut decision_variables = BTreeMap::new();
        for i in 1..=3 {
            decision_variables.insert(VariableID::from(i), DecisionVariable::continuous());
        }

        let mut constraints = BTreeMap::new();
        constraints.insert(
            ConstraintID::from(1),
            Constraint {
                equality: Equality::LessThanOrEqualToZero,
                stage: crate::constraint::CreatedData {
                    function: Function::from(linear!(3) + coeff!(-10.0)),
                },
            },
        );

        let mut instance = Instance::new(
            Sense::Minimize,
            Function::from(((linear!(1) + linear!(2)).unwrap() + linear!(3)).unwrap()),
            decision_variables,
            constraints,
        )
        .unwrap();

        instance
            .relax_constraint(ConstraintID::from(1), "test".to_string(), [])
            .unwrap();

        let fix_state = v1::State {
            entries: [(1, 3.0)].into_iter().collect(),
        };
        instance
            .partial_evaluate(&fix_state, ATol::default())
            .unwrap();

        let substitution = Function::from(linear!(1) + linear!(2));
        instance = instance
            .substitute_one(VariableID::from(3), &substitution)
            .unwrap();

        instance.restore_constraint(ConstraintID::from(1)).unwrap();

        // x3 = x1 + x2, x1 = 3 → x2 - 7
        let restored = instance.constraints().get(&ConstraintID::from(1)).unwrap();
        assert!(!restored.required_ids().contains(&VariableID::from(3)));
        assert!(!restored.required_ids().contains(&VariableID::from(1)));
        assert!(restored.required_ids().contains(&VariableID::from(2)));
    }

    #[test]
    fn test_relax_restore_indicator_constraint() {
        use crate::IndicatorConstraintID;

        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(VariableID::from(1), DecisionVariable::continuous());
        decision_variables.insert(VariableID::from(10), DecisionVariable::binary());

        let mut instance = Instance::new(
            Sense::Minimize,
            Function::from(linear!(1)),
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();

        let ic = crate::IndicatorConstraint::new(
            VariableID::from(10),
            Equality::LessThanOrEqualToZero,
            Function::from(linear!(1) + coeff!(-5.0)),
        );
        instance
            .indicator_constraint_collection
            .insert_with(
                IndicatorConstraintID::from(1),
                ic,
                crate::ConstraintContext::default(),
            )
            .unwrap();

        assert_eq!(instance.indicator_constraints().len(), 1);
        assert!(instance.removed_indicator_constraints().is_empty());

        instance
            .relax_indicator_constraint(IndicatorConstraintID::from(1), "test".to_string(), [])
            .unwrap();
        assert!(instance.indicator_constraints().is_empty());
        assert_eq!(instance.removed_indicator_constraints().len(), 1);

        instance
            .restore_indicator_constraint(IndicatorConstraintID::from(1))
            .unwrap();
        assert_eq!(instance.indicator_constraints().len(), 1);
        assert!(instance.removed_indicator_constraints().is_empty());
    }

    /// Restoring an indicator constraint fails if the indicator variable was fixed.
    #[test]
    fn test_restore_indicator_constraint_fails_when_indicator_fixed() {
        use crate::IndicatorConstraintID;

        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(VariableID::from(1), DecisionVariable::continuous());
        decision_variables.insert(VariableID::from(10), DecisionVariable::binary());

        let mut instance = Instance::new(
            Sense::Minimize,
            Function::from(linear!(1)),
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();

        let ic = crate::IndicatorConstraint::new(
            VariableID::from(10),
            Equality::LessThanOrEqualToZero,
            Function::from(linear!(1) + coeff!(-5.0)),
        );
        instance
            .indicator_constraint_collection
            .insert_with(
                IndicatorConstraintID::from(1),
                ic,
                crate::ConstraintContext::default(),
            )
            .unwrap();

        instance
            .relax_indicator_constraint(IndicatorConstraintID::from(1), "test".to_string(), [])
            .unwrap();

        // Fix the indicator variable
        let fix_state = v1::State {
            entries: [(10, 1.0)].into_iter().collect(),
        };
        instance
            .partial_evaluate(&fix_state, ATol::default())
            .unwrap();

        // Restore should fail
        let result = instance.restore_indicator_constraint(IndicatorConstraintID::from(1));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("indicator variable"));
    }

    /// Restoring an indicator constraint applies pending substitutions to the function
    /// but not to the indicator variable.
    #[test]
    fn test_restore_indicator_constraint_with_fixed_function_variable() {
        use crate::IndicatorConstraintID;

        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(VariableID::from(1), DecisionVariable::continuous());
        decision_variables.insert(VariableID::from(2), DecisionVariable::continuous());
        decision_variables.insert(VariableID::from(10), DecisionVariable::binary());

        let mut instance = Instance::new(
            Sense::Minimize,
            Function::from(linear!(1)),
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();

        // x10 = 1 → x1 + x2 - 5 <= 0
        let ic = crate::IndicatorConstraint::new(
            VariableID::from(10),
            Equality::LessThanOrEqualToZero,
            Function::from(((linear!(1) + linear!(2)).unwrap() + coeff!(-5.0)).unwrap()),
        );
        instance
            .indicator_constraint_collection
            .insert_with(
                IndicatorConstraintID::from(1),
                ic,
                crate::ConstraintContext::default(),
            )
            .unwrap();

        instance
            .relax_indicator_constraint(IndicatorConstraintID::from(1), "test".to_string(), [])
            .unwrap();

        // Fix x1 (function variable, not indicator)
        let fix_state = v1::State {
            entries: [(1, 3.0)].into_iter().collect(),
        };
        instance
            .partial_evaluate(&fix_state, ATol::default())
            .unwrap();

        instance
            .restore_indicator_constraint(IndicatorConstraintID::from(1))
            .unwrap();

        let restored = instance
            .indicator_constraints()
            .get(&IndicatorConstraintID::from(1))
            .unwrap();
        // x1 substituted, x2 remains
        assert!(!restored
            .stage
            .function
            .required_ids()
            .contains(&VariableID::from(1)));
        assert!(restored
            .stage
            .function
            .required_ids()
            .contains(&VariableID::from(2)));
        assert_eq!(restored.indicator_variable, VariableID::from(10));
    }

    /// Restoring an indicator constraint fails if the indicator variable
    /// was substituted (via dependency) while the constraint was removed.
    #[test]
    fn test_restore_indicator_constraint_fails_when_indicator_substituted() {
        use crate::IndicatorConstraintID;

        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(VariableID::from(1), DecisionVariable::continuous());
        decision_variables.insert(VariableID::from(10), DecisionVariable::binary());
        decision_variables.insert(VariableID::from(20), DecisionVariable::binary());

        let mut instance = Instance::new(
            Sense::Minimize,
            Function::from(linear!(1)),
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();

        // x10 = 1 → x1 - 5 <= 0
        let ic = crate::IndicatorConstraint::new(
            VariableID::from(10),
            Equality::LessThanOrEqualToZero,
            Function::from(linear!(1) + coeff!(-5.0)),
        );
        instance
            .indicator_constraint_collection
            .insert_with(
                IndicatorConstraintID::from(1),
                ic,
                crate::ConstraintContext::default(),
            )
            .unwrap();

        // Relax, then substitute indicator variable x10 = x20
        instance
            .relax_indicator_constraint(IndicatorConstraintID::from(1), "test".to_string(), [])
            .unwrap();
        instance = instance
            .substitute_one(VariableID::from(10), &Function::from(linear!(20)))
            .unwrap();

        // Restore should fail because indicator variable x10 has been substituted
        let result = instance.restore_indicator_constraint(IndicatorConstraintID::from(1));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("indicator variable"));
    }
}
