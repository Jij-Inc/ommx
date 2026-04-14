use super::*;
use anyhow::bail;

impl Instance {
    /// Add constraint hints to the instance.
    ///
    /// Constraint hints provide additional information about constraints to help solvers
    /// optimize more efficiently. Hints must only reference **active** constraints
    /// (not removed constraints).
    ///
    /// # Errors
    ///
    /// Returns an error if any hint references a removed constraint or an undefined constraint.
    pub fn add_constraint_hints(
        &mut self,
        constraint_hints: ConstraintHints,
    ) -> anyhow::Result<()> {
        // Validate that no hints reference removed constraints
        for hint in &constraint_hints.one_hot_constraints {
            if self.removed_constraints().contains_key(&hint.id) {
                bail!(
                    "OneHot hint references removed constraint (id={:?}). \
                     Constraint hints can only reference active constraints.",
                    hint.id
                );
            }
        }
        for hint in &constraint_hints.sos1_constraints {
            if self
                .removed_constraints()
                .contains_key(&hint.binary_constraint_id)
            {
                bail!(
                    "Sos1 hint references removed constraint (binary_constraint_id={:?}). \
                     Constraint hints can only reference active constraints.",
                    hint.binary_constraint_id
                );
            }
            for id in &hint.big_m_constraint_ids {
                if self.removed_constraints().contains_key(id) {
                    bail!(
                        "Sos1 hint references removed constraint in big_m_constraint_ids (id={:?}). \
                         Constraint hints can only reference active constraints.",
                        id
                    );
                }
            }
        }

        // Validate constraint_hints using Parse trait (checks variable/constraint existence)
        let hints: v1::ConstraintHints = constraint_hints.into();
        let context = (
            self.decision_variables.clone(),
            self.constraints().clone(),
            self.removed_constraints().clone(),
        );
        let constraint_hints = hints.parse(&context)?;
        self.constraint_hints = constraint_hints;
        Ok(())
    }

    pub fn with_constraint_hints(
        mut self,
        constraint_hints: ConstraintHints,
    ) -> anyhow::Result<Self> {
        self.add_constraint_hints(constraint_hints)?;
        Ok(self)
    }
}

impl ParametricInstance {
    /// Add constraint hints to the parametric instance.
    ///
    /// Constraint hints provide additional information about constraints to help solvers
    /// optimize more efficiently. Hints must only reference **active** constraints
    /// (not removed constraints).
    ///
    /// # Errors
    ///
    /// Returns an error if any hint references a removed constraint or an undefined constraint.
    pub fn add_constraint_hints(
        &mut self,
        constraint_hints: ConstraintHints,
    ) -> anyhow::Result<()> {
        // Validate that no hints reference removed constraints
        for hint in &constraint_hints.one_hot_constraints {
            if self.removed_constraints().contains_key(&hint.id) {
                bail!(
                    "OneHot hint references removed constraint (id={:?}). \
                     Constraint hints can only reference active constraints.",
                    hint.id
                );
            }
        }
        for hint in &constraint_hints.sos1_constraints {
            if self
                .removed_constraints()
                .contains_key(&hint.binary_constraint_id)
            {
                bail!(
                    "Sos1 hint references removed constraint (binary_constraint_id={:?}). \
                     Constraint hints can only reference active constraints.",
                    hint.binary_constraint_id
                );
            }
            for id in &hint.big_m_constraint_ids {
                if self.removed_constraints().contains_key(id) {
                    bail!(
                        "Sos1 hint references removed constraint in big_m_constraint_ids (id={:?}). \
                         Constraint hints can only reference active constraints.",
                        id
                    );
                }
            }
        }

        // Validate constraint_hints using Parse trait (checks variable/constraint existence)
        let hints: v1::ConstraintHints = constraint_hints.into();
        let context = (
            self.decision_variables.clone(),
            self.constraints().clone(),
            self.removed_constraints().clone(),
        );
        let constraint_hints = hints.parse(&context)?;
        self.constraint_hints = constraint_hints;
        Ok(())
    }

    pub fn with_constraint_hints(
        mut self,
        constraint_hints: ConstraintHints,
    ) -> anyhow::Result<Self> {
        self.add_constraint_hints(constraint_hints)?;
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coeff,
        constraint::{Constraint, ConstraintID},
        constraint_hints::{ConstraintHints, OneHot},
        linear, DecisionVariable, Sense, VariableID,
    };
    use maplit::btreemap;
    use std::collections::BTreeSet;

    #[test]
    fn test_instance_add_constraint_hints() {
        // Test adding constraint hints to an instance
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
        };

        let objective = (linear!(1) + coeff!(1.0)).into();
        let constraints = btreemap! {
            ConstraintID::from(1) => Constraint::equal_to_zero(ConstraintID::from(1), (linear!(1) + coeff!(1.0)).into()),
        };

        let mut variables = BTreeSet::new();
        variables.insert(VariableID::from(1));
        variables.insert(VariableID::from(2));

        let one_hot = OneHot {
            id: ConstraintID::from(1),
            variables,
        };

        let constraint_hints = ConstraintHints {
            one_hot_constraints: vec![one_hot],
            sos1_constraints: vec![],
        };
        let instance = Instance::new(Sense::Minimize, objective, decision_variables, constraints)
            .unwrap()
            .with_constraint_hints(constraint_hints)
            .unwrap();

        assert_eq!(instance.constraint_hints.one_hot_constraints.len(), 1);
    }

    #[test]
    fn test_parametric_instance_add_constraint_hints() {
        // Test adding constraint hints to a parametric instance
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
        };

        let parameters = btreemap! {
            VariableID::from(100) => v1::Parameter { id: 100, name: Some("p1".to_string()), ..Default::default() },
        };

        let objective = (linear!(1) + coeff!(1.0)).into();
        let constraints = btreemap! {
            ConstraintID::from(1) => Constraint::equal_to_zero(ConstraintID::from(1), (linear!(1) + coeff!(1.0)).into()),
        };

        let mut variables = BTreeSet::new();
        variables.insert(VariableID::from(1));
        variables.insert(VariableID::from(2));

        let one_hot = OneHot {
            id: ConstraintID::from(1),
            variables,
        };

        let constraint_hints = ConstraintHints {
            one_hot_constraints: vec![one_hot],
            sos1_constraints: vec![],
        };

        let parametric_instance = ParametricInstance::new(
            Sense::Minimize,
            objective,
            decision_variables,
            parameters,
            constraints,
        )
        .unwrap()
        .with_constraint_hints(constraint_hints)
        .unwrap();

        assert_eq!(
            parametric_instance
                .constraint_hints
                .one_hot_constraints
                .len(),
            1
        );
    }

    #[test]
    fn test_add_constraint_hints_error_on_removed_constraint() {
        // Test that adding constraint hints referencing a removed constraint fails
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
        };

        let objective = (linear!(1) + coeff!(1.0)).into();
        let constraints = btreemap! {
            ConstraintID::from(1) => Constraint::equal_to_zero(ConstraintID::from(1), (linear!(1) + coeff!(1.0)).into()),
            ConstraintID::from(2) => Constraint::equal_to_zero(ConstraintID::from(2), (linear!(2) + coeff!(1.0)).into()),
        };
        let mut instance =
            Instance::new(Sense::Minimize, objective, decision_variables, constraints).unwrap();

        // Relax constraint 2 (move to removed_constraints)
        instance
            .relax_constraint(ConstraintID::from(2), "test".to_string(), [])
            .unwrap();

        // Try to add a hint that references the removed constraint
        let mut variables = BTreeSet::new();
        variables.insert(VariableID::from(1));
        variables.insert(VariableID::from(2));

        let one_hot = OneHot {
            id: ConstraintID::from(2), // References removed constraint
            variables,
        };

        let constraint_hints = ConstraintHints {
            one_hot_constraints: vec![one_hot],
            sos1_constraints: vec![],
        };

        // This should fail because the hint references a removed constraint
        let result = instance.add_constraint_hints(constraint_hints);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("removed constraint"),
            "Error message should mention 'removed constraint': {}",
            err_msg
        );
    }
}
