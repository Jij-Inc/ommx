use crate::{
    parse::RawParseError, v1, Constraint, ConstraintID, DecisionVariable, Function,
    RemovedConstraint, VariableID,
};
use anyhow::{bail, Context as _};
use std::collections::{BTreeSet, HashMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Sense {
    Minimize,
    Maximize,
}

impl TryFrom<v1::instance::Sense> for Sense {
    type Error = RawParseError;
    fn try_from(value: v1::instance::Sense) -> Result<Self, Self::Error> {
        match value {
            v1::instance::Sense::Minimize => Ok(Self::Minimize),
            v1::instance::Sense::Maximize => Ok(Self::Maximize),
            v1::instance::Sense::Unspecified => Err(RawParseError::UnspecifiedEnum {
                enum_name: "ommx.v1.instance.Sense",
            }),
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
    type Error = anyhow::Error;
    fn try_from(value: v1::Instance) -> Result<Self, Self::Error> {
        let sense = value.sense().try_into()?;

        let objective = value
            .objective
            .ok_or(RawParseError::MissingField {
                message: "ommx.v1.Instance",
                field: "objective",
            })?
            .try_into()?;

        let mut decision_variables = HashMap::new();
        for v in value.decision_variables {
            let v: DecisionVariable = v.try_into()?;
            let id = v.id;
            if decision_variables.insert(id, v).is_some() {
                return Err(RawParseError::DuplicatedVariableID { id }.into());
            }
        }

        let mut constraints = HashMap::new();
        for c in value.constraints {
            let c: Constraint = c.try_into()?;
            let id = c.id;
            if constraints.insert(id, c).is_some() {
                return Err(RawParseError::DuplicatedConstraintID { id }.into());
            }
        }

        let mut removed_constraints = HashMap::new();
        for c in value.removed_constraints {
            let c: RemovedConstraint = c.try_into()?;
            let id = c.constraint.id;
            if constraints.contains_key(&id) {
                return Err(RawParseError::DuplicatedConstraintID { id })
                    .context("ID of removed constraint is duplicated with existing constraint");
            }
            if removed_constraints.insert(id, c).is_some() {
                return Err(RawParseError::DuplicatedConstraintID { id }).context(
                    "ID of removed constraint is duplicated with another removed constraint",
                );
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
                as_variable_id(id).context("Unknown variable ID is used for dependent variable")?,
                f.try_into()
                    .context("Function for dependent variable is invalid")?,
            );
        }

        let constraint_hints = if let Some(hints) = value.constraint_hints {
            let mut one_hot_constraints = Vec::new();
            for onehot in hints.one_hot_constraints {
                let constraint_id =
                    as_constraint_id(onehot.constraint_id).context("Undefined constraint ID")?;
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
