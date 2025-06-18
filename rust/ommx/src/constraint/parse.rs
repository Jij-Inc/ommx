use std::collections::BTreeMap;

use super::*;
use crate::{
    parse::{Parse, ParseError, RawParseError},
    v1, InstanceError,
};
use anyhow::Result;

impl Parse for v1::Equality {
    type Output = Equality;
    type Context = ();
    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        match self {
            v1::Equality::EqualToZero => Ok(Equality::EqualToZero),
            v1::Equality::LessThanOrEqualToZero => Ok(Equality::LessThanOrEqualToZero),
            _ => Err(RawParseError::UnspecifiedEnum {
                enum_name: "ommx.v1.Equality",
            }
            .into()),
        }
    }
}

impl From<Equality> for v1::Equality {
    fn from(value: Equality) -> Self {
        match value {
            Equality::EqualToZero => v1::Equality::EqualToZero,
            Equality::LessThanOrEqualToZero => v1::Equality::LessThanOrEqualToZero,
        }
    }
}

impl From<Equality> for i32 {
    fn from(equality: Equality) -> Self {
        v1::Equality::from(equality).into()
    }
}

impl Parse for v1::Constraint {
    type Output = Constraint;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.Constraint";
        Ok(Constraint {
            id: ConstraintID(self.id),
            equality: self.equality().parse_as(&(), message, "equality")?,
            function: self
                .function
                .ok_or(RawParseError::MissingField {
                    message,
                    field: "function",
                })?
                .parse_as(&(), message, "function")?,
            name: self.name,
            subscripts: self.subscripts,
            parameters: self.parameters.into_iter().collect(),
            description: self.description,
        })
    }
}

impl Parse for v1::RemovedConstraint {
    type Output = RemovedConstraint;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.RemovedConstraint";
        Ok(RemovedConstraint {
            constraint: self
                .constraint
                .ok_or(RawParseError::MissingField {
                    message,
                    field: "constraint",
                })?
                .parse_as(&(), message, "constraint")?,
            removed_reason: self.removed_reason,
            removed_reason_parameters: self.removed_reason_parameters.into_iter().collect(),
        })
    }
}

impl Parse for Vec<v1::Constraint> {
    type Output = BTreeMap<ConstraintID, Constraint>;
    type Context = ();
    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let mut constraints = BTreeMap::default();
        for c in self {
            let c: Constraint = c.parse(&())?;
            let id = c.id;
            if constraints.insert(id, c).is_some() {
                return Err(
                    RawParseError::InstanceError(InstanceError::DuplicatedConstraintID { id })
                        .into(),
                );
            }
        }
        Ok(constraints)
    }
}

impl Parse for Vec<v1::RemovedConstraint> {
    type Output = BTreeMap<ConstraintID, RemovedConstraint>;
    type Context = BTreeMap<ConstraintID, Constraint>;
    fn parse(self, constraints: &Self::Context) -> Result<Self::Output, ParseError> {
        let mut removed_constraints = BTreeMap::default();
        for c in self {
            let c: RemovedConstraint = c.parse(&())?;
            let id = c.constraint.id;
            if constraints.contains_key(&id) {
                return Err(
                    RawParseError::InstanceError(InstanceError::DuplicatedConstraintID { id })
                        .into(),
                );
            }
            if removed_constraints.insert(id, c).is_some() {
                return Err(
                    RawParseError::InstanceError(InstanceError::DuplicatedConstraintID { id })
                        .into(),
                );
            }
        }
        Ok(removed_constraints)
    }
}

impl Parse for v1::EvaluatedConstraint {
    type Output = EvaluatedConstraint;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.EvaluatedConstraint";

        let equality = self.equality().parse_as(&(), message, "equality")?;

        let metadata = ConstraintMetadata {
            name: self.name,
            subscripts: self.subscripts,
            parameters: self.parameters.into_iter().collect(),
            description: self.description,
            used_decision_variable_ids: self.used_decision_variable_ids,
            removed_reason: self.removed_reason,
            removed_reason_parameters: self.removed_reason_parameters.into_iter().collect(),
        };

        Ok(EvaluatedConstraint {
            id: ConstraintID(self.id),
            equality,
            metadata,
            evaluated_value: self.evaluated_value,
            dual_variable: self.dual_variable,
        })
    }
}

impl Parse for v1::SampledConstraint {
    type Output = SampledConstraint;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.SampledConstraint";

        let equality = self.equality().parse_as(&(), message, "equality")?;

        // Parse evaluated_values
        let evaluated_values = self
            .evaluated_values
            .ok_or(RawParseError::MissingField {
                message,
                field: "evaluated_values",
            })?
            .parse_as(&(), message, "evaluated_values")?;

        let metadata = ConstraintMetadata {
            name: self.name,
            subscripts: self.subscripts,
            parameters: self.parameters.into_iter().collect(),
            description: self.description,
            used_decision_variable_ids: self.used_decision_variable_ids,
            removed_reason: self.removed_reason,
            removed_reason_parameters: self.removed_reason_parameters.into_iter().collect(),
        };

        Ok(SampledConstraint {
            id: ConstraintID(self.id),
            equality,
            metadata,
            evaluated_values,
            dual_variables: None, // v1::SampledConstraint doesn't have dual_variables field
            feasible: self.feasible.into_iter().collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v1;

    #[test]
    fn error_message() {
        let out: Result<RemovedConstraint, ParseError> = v1::RemovedConstraint {
            constraint: Some(v1::Constraint {
                id: 1,
                function: Some(v1::Function { function: None }),
                equality: v1::Equality::EqualToZero as i32,
                ..Default::default()
            }),
            removed_reason: "reason".to_string(),
            removed_reason_parameters: Default::default(),
        }
        .parse(&());

        insta::assert_snapshot!(out.unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.RemovedConstraint[constraint]
          └─ommx.v1.Constraint[function]
        Unsupported ommx.v1.Function is found. It is created by a newer version of OMMX SDK.
        "###);
    }

    #[test]
    fn test_evaluated_constraint_parse() {
        let v1_constraint = v1::EvaluatedConstraint {
            id: 42,
            equality: v1::Equality::EqualToZero as i32,
            evaluated_value: 1.5,
            used_decision_variable_ids: vec![1, 2, 3],
            subscripts: vec![10, 20],
            parameters: [("key1".to_string(), "value1".to_string())]
                .iter()
                .cloned()
                .collect(),
            name: Some("test_constraint".to_string()),
            description: Some("A test constraint".to_string()),
            dual_variable: Some(0.5),
            removed_reason: None,
            removed_reason_parameters: Default::default(),
        };

        let parsed: EvaluatedConstraint = v1_constraint.parse(&()).unwrap();

        assert_eq!(parsed.id, ConstraintID(42));
        assert_eq!(parsed.equality, Equality::EqualToZero);
        assert_eq!(parsed.evaluated_value, 1.5);
        assert_eq!(parsed.dual_variable, Some(0.5));
        assert_eq!(parsed.metadata.name, Some("test_constraint".to_string()));
        assert_eq!(
            parsed.metadata.description,
            Some("A test constraint".to_string())
        );
        assert_eq!(parsed.metadata.used_decision_variable_ids, vec![1, 2, 3]);
        assert_eq!(parsed.metadata.subscripts, vec![10, 20]);
    }
}
