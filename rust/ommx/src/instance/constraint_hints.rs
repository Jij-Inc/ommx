use super::{Instance, ParametricInstance};
use crate::{parse::Parse, v1, ConstraintHints};

impl Instance {
    pub fn add_constraint_hints(
        &mut self,
        constraint_hints: ConstraintHints,
    ) -> anyhow::Result<()> {
        // Validate constraint_hints using Parse trait
        let hints: v1::ConstraintHints = constraint_hints.into();
        let context = (
            self.decision_variables.clone(),
            self.constraints.clone(),
            self.removed_constraints.clone(),
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
    pub fn add_constraint_hints(
        &mut self,
        constraint_hints: ConstraintHints,
    ) -> anyhow::Result<()> {
        // Validate constraint_hints using Parse trait
        let hints: v1::ConstraintHints = constraint_hints.into();
        let context = (
            self.decision_variables.clone(),
            self.constraints.clone(),
            self.removed_constraints.clone(),
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
        linear,
        v1::State,
        DecisionVariable, Evaluate, Sense, VariableID,
    };
    use maplit::{btreemap, btreeset, hashmap};
    use std::collections::{BTreeMap, BTreeSet};

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

        let one_hot = OneHot::new(ConstraintID::from(1), variables);
        let constraint_hints = ConstraintHints::new(vec![one_hot], vec![]);

        let instance = Instance::new(Sense::Minimize, objective, decision_variables, constraints)
            .unwrap()
            .with_constraint_hints(constraint_hints)
            .unwrap();

        assert_eq!(instance.constraint_hints.one_hot_constraints().len(), 1);
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

        let one_hot = OneHot::new(ConstraintID::from(1), variables);
        let constraint_hints = ConstraintHints::new(vec![one_hot], vec![]);

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
                .one_hot_constraints()
                .len(),
            1
        );
    }

    #[test]
    fn test_instance_partial_evaluate_constraint_hints() {
        // Test that Instance.partial_evaluate applies to constraint hints
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

        let objective = (linear!(1) + coeff!(1.0)).into();
        let constraints = btreemap! {
            ConstraintID::from(1) => Constraint::equal_to_zero(ConstraintID::from(1), (linear!(1) + coeff!(1.0)).into()),
        };

        let constraint_hints = ConstraintHints::new(
            vec![OneHot::new(
                ConstraintID::from(1),
                btreeset! {
                    VariableID::from(1),
                    VariableID::from(2),
                    VariableID::from(3),
                },
            )],
            vec![],
        );

        let mut instance =
            Instance::new(Sense::Minimize, objective, decision_variables, constraints)
                .unwrap()
                .with_constraint_hints(constraint_hints)
                .unwrap();

        // Verify initial state
        assert_eq!(instance.constraint_hints.one_hot_constraints().len(), 1);
        assert_eq!(
            instance.constraint_hints.one_hot_constraints()[0]
                .variables()
                .len(),
            3
        );

        // Apply partial evaluation: set variable 1 to 0, variable 2 to 1
        let state = State {
            entries: hashmap! {
                1 => 0.0,  // Should be removed
                2 => 1.0,  // Should cause hint discard
            },
        };

        instance
            .partial_evaluate(&state, crate::ATol::default())
            .unwrap();

        // The OneHot constraint hint should be discarded due to non-zero value
        assert!(instance.constraint_hints.one_hot_constraints().is_empty());
    }
}
