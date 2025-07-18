use super::{parse::*, Instance, ParametricInstance};
use crate::{
    parse::{Parse, ParseError, RawParseError},
    v1::{self},
    Constraint, ConstraintID, DecisionVariable, InstanceError, RemovedConstraint, VariableID,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OneHot {
    pub id: ConstraintID,
    pub variables: BTreeSet<VariableID>,
}

impl Parse for v1::OneHot {
    type Output = OneHot;
    type Context = (
        BTreeMap<VariableID, DecisionVariable>,
        BTreeMap<ConstraintID, Constraint>,
        BTreeMap<ConstraintID, RemovedConstraint>,
    );
    fn parse(
        self,
        (decision_variable, constraints, removed_constraints): &Self::Context,
    ) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.OneHot";
        let constraint_id = as_constraint_id(constraints, removed_constraints, self.constraint_id)
            .map_err(|e| e.context(message, "constraint_id"))?;
        let mut variables = BTreeSet::new();
        for v in &self.decision_variables {
            let id = as_variable_id(decision_variable, *v)
                .map_err(|e| e.context(message, "decision_variables"))?;
            if !variables.insert(id) {
                return Err(
                    RawParseError::InstanceError(InstanceError::NonUniqueVariableID { id })
                        .context(message, "decision_variables"),
                );
            }
        }
        Ok(OneHot {
            id: constraint_id,
            variables,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sos1 {
    pub binary_constraint_id: ConstraintID,
    pub big_m_constraint_ids: BTreeSet<ConstraintID>,
    pub variables: BTreeSet<VariableID>,
}

impl Parse for v1::Sos1 {
    type Output = Sos1;
    type Context = (
        BTreeMap<VariableID, DecisionVariable>,
        BTreeMap<ConstraintID, Constraint>,
        BTreeMap<ConstraintID, RemovedConstraint>,
    );
    fn parse(
        self,
        (decision_variable, constraints, removed_constraints): &Self::Context,
    ) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.Sos1";
        let binary_constraint_id =
            as_constraint_id(constraints, removed_constraints, self.binary_constraint_id)
                .map_err(|e| e.context(message, "binary_constraint_id"))?;
        let mut big_m_constraint_ids = BTreeSet::new();
        for id in &self.big_m_constraint_ids {
            let id = as_constraint_id(constraints, removed_constraints, *id)
                .map_err(|e| e.context(message, "big_m_constraint_ids"))?;
            if !big_m_constraint_ids.insert(id) {
                return Err(
                    RawParseError::InstanceError(InstanceError::NonUniqueConstraintID { id })
                        .context(message, "big_m_constraint_ids"),
                );
            }
        }
        let mut variables = BTreeSet::new();
        for id in &self.decision_variables {
            let id = as_variable_id(decision_variable, *id)
                .map_err(|e| e.context(message, "decision_variables"))?;
            if !variables.insert(id) {
                return Err(
                    RawParseError::InstanceError(InstanceError::NonUniqueVariableID { id })
                        .context(message, "decision_variables"),
                );
            }
        }
        Ok(Sos1 {
            binary_constraint_id,
            big_m_constraint_ids,
            variables,
        })
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ConstraintHints {
    pub one_hot_constraints: Vec<OneHot>,
    pub sos1_constraints: Vec<Sos1>,
}

impl ConstraintHints {
    pub fn is_empty(&self) -> bool {
        self.one_hot_constraints.is_empty() && self.sos1_constraints.is_empty()
    }
}

impl Parse for v1::ConstraintHints {
    type Output = ConstraintHints;
    type Context = (
        BTreeMap<VariableID, DecisionVariable>,
        BTreeMap<ConstraintID, Constraint>,
        BTreeMap<ConstraintID, RemovedConstraint>,
    );
    fn parse(self, context: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.ConstraintHints";
        let one_hot_constraints = self
            .one_hot_constraints
            .into_iter()
            .map(|c| c.parse_as(context, message, "one_hot_constraints"))
            .collect::<Result<Vec<_>, ParseError>>()?;
        let sos1_constraints = self
            .sos1_constraints
            .into_iter()
            .map(|c| c.parse_as(context, message, "sos1_constraints"))
            .collect::<Result<_, ParseError>>()?;
        Ok(ConstraintHints {
            one_hot_constraints,
            sos1_constraints,
        })
    }
}

impl From<OneHot> for v1::OneHot {
    fn from(value: OneHot) -> Self {
        Self {
            constraint_id: *value.id,
            decision_variables: value.variables.into_iter().map(|v| *v).collect(),
        }
    }
}

impl From<Sos1> for v1::Sos1 {
    fn from(value: Sos1) -> Self {
        Self {
            binary_constraint_id: *value.binary_constraint_id,
            big_m_constraint_ids: value.big_m_constraint_ids.into_iter().map(|c| *c).collect(),
            decision_variables: value.variables.into_iter().map(|v| *v).collect(),
        }
    }
}

impl From<ConstraintHints> for v1::ConstraintHints {
    fn from(value: ConstraintHints) -> Self {
        Self {
            one_hot_constraints: value
                .one_hot_constraints
                .into_iter()
                .map(|oh| oh.into())
                .collect(),
            sos1_constraints: value
                .sos1_constraints
                .into_iter()
                .map(|s| s.into())
                .collect(),
        }
    }
}

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
