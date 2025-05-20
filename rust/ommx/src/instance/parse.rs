use super::*;
use crate::{
    parse::{Parse, ParseError, RawParseError},
    v1::{self},
    Constraint, ConstraintID, DecisionVariable, VariableID,
};

impl Parse for v1::instance::Sense {
    type Output = Sense;
    type Context = ();
    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        match self {
            v1::instance::Sense::Minimize => Ok(Sense::Minimize),
            v1::instance::Sense::Maximize => Ok(Sense::Maximize),
            v1::instance::Sense::Unspecified => Err(RawParseError::UnspecifiedEnum {
                enum_name: "ommx.v1.instance.Sense",
            }
            .into()),
        }
    }
}

impl From<Sense> for v1::instance::Sense {
    fn from(value: Sense) -> Self {
        match value {
            Sense::Minimize => v1::instance::Sense::Minimize,
            Sense::Maximize => v1::instance::Sense::Maximize,
        }
    }
}

impl From<Sense> for i32 {
    fn from(value: Sense) -> Self {
        v1::instance::Sense::from(value).into()
    }
}

impl TryFrom<v1::Instance> for Instance {
    type Error = ParseError;
    fn try_from(value: v1::Instance) -> Result<Self, Self::Error> {
        let message = "ommx.v1.Instance";
        let sense = value.sense().parse_as(&(), message, "sense")?;

        let decision_variables =
            value
                .decision_variables
                .parse_as(&(), message, "decision_variables")?;

        let objective = value
            .objective
            .ok_or(RawParseError::MissingField {
                message,
                field: "objective",
            })?
            .parse_as(&(), message, "objective")?;

        let constraints = value.constraints.parse_as(&(), message, "constraints")?;
        let removed_constraints =
            value
                .removed_constraints
                .parse_as(&constraints, message, "removed_constraints")?;

        let mut decision_variable_dependency = BTreeMap::default();
        for (id, f) in value.decision_variable_dependency {
            decision_variable_dependency.insert(
                as_variable_id(&decision_variables, id)
                    .map_err(|e| e.context(message, "decision_variable_dependency"))?,
                f.parse_as(&(), message, "decision_variable_dependency")?,
            );
        }

        let context = (decision_variables, constraints);
        let constraint_hints = if let Some(hints) = value.constraint_hints {
            hints.parse_as(&context, message, "constraint_hints")?
        } else {
            Default::default()
        };
        let (decision_variables, constraints) = context;

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

pub(super) fn as_constraint_id(
    constraints: &BTreeMap<ConstraintID, Constraint>,
    id: u64,
) -> Result<ConstraintID, ParseError> {
    let id = ConstraintID::from(id);
    if !constraints.contains_key(&id) {
        return Err(RawParseError::UndefinedConstraintID { id }.into());
    }
    Ok(id)
}

pub(super) fn as_variable_id(
    decision_variables: &BTreeMap<VariableID, DecisionVariable>,
    id: u64,
) -> Result<VariableID, ParseError> {
    let id = VariableID::from(id);
    if !decision_variables.contains_key(&id) {
        return Err(RawParseError::UndefinedVariableID { id }.into());
    }
    Ok(id)
}
