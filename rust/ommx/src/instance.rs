use crate::{
    parse::{Parse, ParseError, RawParseError},
    v1::{self, decision_variable},
    Constraint, ConstraintID, DecisionVariable, Function, RemovedConstraint, VariableID,
};
use anyhow::{bail, Context as _};
use serde::de;
use std::collections::{BTreeSet, HashMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Sense {
    Minimize,
    Maximize,
}

impl TryFrom<v1::instance::Sense> for Sense {
    type Error = ParseError;
    fn try_from(value: v1::instance::Sense) -> Result<Self, Self::Error> {
        match value {
            v1::instance::Sense::Minimize => Ok(Self::Minimize),
            v1::instance::Sense::Maximize => Ok(Self::Maximize),
            v1::instance::Sense::Unspecified => Err(RawParseError::UnspecifiedEnum {
                enum_name: "ommx.v1.instance.Sense",
            }
            .into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OneHotConstraint {
    pub id: ConstraintID,
    pub variables: BTreeSet<VariableID>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SOS1Constraints {
    pub binary_constraint_id: ConstraintID,
    pub big_m_constraint_ids: BTreeSet<ConstraintID>,
    pub variables: BTreeSet<VariableID>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ConstraintHints {
    pub one_hot_constraints: Vec<OneHotConstraint>,
    pub sos1_constraints: Vec<SOS1Constraints>,
}

impl v1::ConstraintHints {
    fn parse(self) -> ConstraintHints {
        todo!()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Instance {
    sense: Sense,
    objective: Function,
    decision_variables: HashMap<VariableID, DecisionVariable>,
    constraints: HashMap<ConstraintID, Constraint>,
    removed_constraints: HashMap<ConstraintID, RemovedConstraint>,
    decision_variable_dependency: HashMap<VariableID, Function>,
    parameters: Option<v1::Parameters>,
    description: Option<v1::instance::Description>,
    constraint_hints: ConstraintHints,
}

impl TryFrom<v1::Instance> for Instance {
    type Error = ParseError;
    fn try_from(value: v1::Instance) -> Result<Self, Self::Error> {
        let message = "ommx.v1.Instance";
        let sense = value.sense().parse(message, "sense")?;

        let objective = value
            .objective
            .ok_or(RawParseError::MissingField {
                message,
                field: "objective",
            })?
            .parse(message, "objective")?;

        let mut decision_variables = HashMap::new();
        for v in value.decision_variables {
            let v: DecisionVariable = v.parse(message, "decision_variables")?;
            let id = v.id;
            if decision_variables.insert(id, v).is_some() {
                return Err(RawParseError::DuplicatedVariableID { id }
                    .context(message, "decision_variables"));
            }
        }

        let mut constraints = HashMap::new();
        for c in value.constraints {
            let c: Constraint = c.parse(message, "constraints")?;
            let id = c.id;
            if constraints.insert(id, c).is_some() {
                return Err(
                    RawParseError::DuplicatedConstraintID { id }.context(message, "constraints")
                );
            }
        }

        let mut removed_constraints = HashMap::new();
        for c in value.removed_constraints {
            let c: RemovedConstraint = c.try_into()?;
            let id = c.constraint.id;
            if constraints.contains_key(&id) {
                return Err(RawParseError::DuplicatedConstraintID { id }
                    .context(message, "removed_constraints"));
            }
            if removed_constraints.insert(id, c).is_some() {
                return Err(RawParseError::DuplicatedConstraintID { id }
                    .context(message, "removed_constraints"));
            }
        }

        let as_variable_id = |id: u64| {
            let id = VariableID::from(id);
            if !decision_variables.contains_key(&id) {
                return Err(RawParseError::UndefinedVariableID { id });
            }
            Ok(id)
        };
        let as_constraint_id = |id: u64| {
            let id = ConstraintID::from(id);
            if !constraints.contains_key(&id) {
                return Err(RawParseError::UndefinedConstraintID { id });
            }
            Ok(id)
        };

        let mut decision_variable_dependency = HashMap::new();
        for (id, f) in value.decision_variable_dependency {
            decision_variable_dependency.insert(
                as_variable_id(id)
                    .map_err(|e| e.context(message, "decision_variable_dependency"))?,
                f.parse(message, "decision_variable_dependency")?,
            );
        }

        let constraint_hints = if let Some(hints) = value.constraint_hints {
            let mut one_hot_constraints = Vec::new();
            for onehot in hints.one_hot_constraints {
                let constraint_id = as_constraint_id(onehot.constraint_id).map_err(|e| {
                    e.context("ommx.v1.OneHotConstraint", "constraint_id")
                        .context("ommx.v1.ConstraintHints", "one_hot_constraints")
                        .context(message, "constraint_hints")
                })?;
                let mut variables = BTreeSet::new();
                for v in &onehot.decision_variables {
                    let id = as_variable_id(*v)?;
                    if !variables.insert(id) {
                        bail!("One-hot constraint {constraint_id:?} contains duplicated decision variable {id:?}");
                    }
                }
                one_hot_constraints.push(OneHotConstraint {
                    id: constraint_id,
                    variables,
                });
            }
            let mut sos1_constraints = Vec::new();
            for sos1 in hints.sos1_constraints {
                let variables = sos1
                    .decision_variables
                    .into_iter()
                    .map(as_variable_id)
                    .collect::<Result<_, RawParseError>>()?;
                let big_m_constraint_ids = sos1
                    .big_m_constraint_ids
                    .into_iter()
                    .map(as_constraint_id)
                    .collect::<Result<_, RawParseError>>()?;
                sos1_constraints.push(SOS1Constraints {
                    binary_constraint_id: as_constraint_id(sos1.binary_constraint_id)?,
                    big_m_constraint_ids,
                    variables,
                });
            }
            ConstraintHints {
                one_hot_constraints,
                sos1_constraints,
            }
        } else {
            ConstraintHints::default()
        };

        Ok(Self {
            sense,
            objective,
            constraints,
            decision_variables,
            removed_constraints,
            decision_variable_dependency,
            parameters: value.parameters,
            description: value.description,
            constraint_hints,
        })
    }
}
