use super::*;

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
}
