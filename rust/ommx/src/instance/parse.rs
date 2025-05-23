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

impl From<Constraint> for v1::Constraint {
    fn from(value: Constraint) -> Self {
        Self {
            id: *value.id,
            equality: value.equality.into(),
            function: Some(value.function.into()),
            name: value.name,
            subscripts: value.subscripts,
            parameters: value.parameters.into_iter().collect(),
            description: value.description,
        }
    }
}

impl From<RemovedConstraint> for v1::RemovedConstraint {
    fn from(value: RemovedConstraint) -> Self {
        Self {
            constraint: Some(value.constraint.into()),
            removed_reason: value.removed_reason,
            removed_reason_parameters: value.removed_reason_parameters.into_iter().collect(),
        }
    }
}

impl Parse for v1::Instance {
    type Output = Instance;
    type Context = ();
    fn parse(self, _context: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.Instance";
        let sense = self.sense().parse_as(&(), message, "sense")?;

        let decision_variables =
            self.decision_variables
                .parse_as(&(), message, "decision_variables")?;

        let objective = self
            .objective
            .ok_or(RawParseError::MissingField {
                message,
                field: "objective",
            })?
            .parse_as(&(), message, "objective")?;

        let constraints = self.constraints.parse_as(&(), message, "constraints")?;
        let removed_constraints =
            self.removed_constraints
                .parse_as(&constraints, message, "removed_constraints")?;

        let mut decision_variable_dependency = BTreeMap::default();
        for (id, f) in self.decision_variable_dependency {
            decision_variable_dependency.insert(
                as_variable_id(&decision_variables, id)
                    .map_err(|e| e.context(message, "decision_variable_dependency"))?,
                f.parse_as(&(), message, "decision_variable_dependency")?,
            );
        }

        let context = (decision_variables, constraints);
        let constraint_hints = if let Some(hints) = self.constraint_hints {
            hints.parse_as(&context, message, "constraint_hints")?
        } else {
            Default::default()
        };
        let (decision_variables, constraints) = context;

        Ok(Instance {
            sense,
            objective,
            constraints,
            decision_variables,
            removed_constraints,
            decision_variable_dependency,
            parameters: self.parameters,
            description: self.description,
            constraint_hints,
        })
    }
}

impl TryFrom<v1::Instance> for Instance {
    type Error = ParseError;
    fn try_from(value: v1::Instance) -> Result<Self, Self::Error> {
        value.parse(&())
    }
}

impl From<Instance> for v1::Instance {
    fn from(value: Instance) -> Self {
        let decision_variables = value
            .decision_variables
            .into_values()
            .map(|dv| dv.into())
            .collect();
        let constraints = value.constraints.into_values().map(|c| c.into()).collect();
        let removed_constraints = value
            .removed_constraints
            .into_values()
            .map(|rc| rc.into())
            .collect();
        let decision_variable_dependency = value
            .decision_variable_dependency
            .into_iter()
            .map(|(id, dep)| (id.into(), dep.into()))
            .collect();
        Self {
            sense: v1::instance::Sense::from(value.sense).into(),
            decision_variables,
            objective: Some(value.objective.into()),
            constraints,
            removed_constraints,
            decision_variable_dependency,
            parameters: value.parameters,
            description: value.description,
            constraint_hints: Some(value.constraint_hints.into()),
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instance::Instance;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn instance_roundtrip(original_instance in Instance::arbitrary()) {
            let v1_instance: v1::Instance = original_instance.clone().into();
            let roundtripped_instance = Instance::try_from(v1_instance).unwrap();
            assert_eq!(original_instance, roundtripped_instance);
        }
    }
}
