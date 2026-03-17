use std::collections::BTreeMap;

use super::*;
use crate::{
    parse::{Parse, ParseError, RawParseError},
    v1, InstanceError, VariableID,
};
use anyhow::Result;

impl Parse for v1::NamedFunction {
    type Output = NamedFunction;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.NamedFunction";
        Ok(NamedFunction {
            id: NamedFunctionID(self.id),
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

impl Parse for Vec<v1::NamedFunction> {
    type Output = BTreeMap<NamedFunctionID, NamedFunction>;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let mut named_functions = BTreeMap::new();
        for named_function in self {
            let named_function = named_function.parse(&())?;
            let id = named_function.id;
            if named_functions.insert(id, named_function).is_some() {
                return Err(RawParseError::InstanceError(
                    InstanceError::DuplicatedNamedFunctionID { id },
                )
                .into());
            }
        }
        Ok(named_functions)
    }
}

impl From<NamedFunction> for v1::NamedFunction {
    fn from(
        NamedFunction {
            id,
            function,
            name,
            subscripts,
            parameters,
            description,
        }: NamedFunction,
    ) -> Self {
        Self {
            id: id.into_inner(),
            function: Some(function.into()),
            name,
            subscripts,
            parameters: parameters.into_iter().collect(),
            description,
        }
    }
}

impl From<EvaluatedNamedFunction> for v1::EvaluatedNamedFunction {
    fn from(
        EvaluatedNamedFunction {
            id,
            evaluated_value,
            name,
            subscripts,
            parameters,
            description,
            used_decision_variable_ids,
        }: EvaluatedNamedFunction,
    ) -> Self {
        Self {
            id: id.into_inner(),
            evaluated_value,
            name,
            subscripts,
            parameters: parameters.into_iter().collect(),
            description,
            used_decision_variable_ids: used_decision_variable_ids
                .into_iter()
                .map(|id| id.into_inner())
                .collect(),
        }
    }
}

impl Parse for v1::EvaluatedNamedFunction {
    type Output = EvaluatedNamedFunction;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        Ok(EvaluatedNamedFunction {
            id: NamedFunctionID(self.id),
            evaluated_value: self.evaluated_value,
            name: self.name,
            subscripts: self.subscripts,
            parameters: self.parameters.into_iter().collect(),
            description: self.description,
            used_decision_variable_ids: self
                .used_decision_variable_ids
                .into_iter()
                .map(VariableID::from)
                .collect(),
        })
    }
}

impl From<SampledNamedFunction> for v1::SampledNamedFunction {
    fn from(
        SampledNamedFunction {
            id,
            evaluated_values,
            name,
            subscripts,
            parameters,
            description,
            used_decision_variable_ids,
        }: SampledNamedFunction,
    ) -> Self {
        Self {
            id: id.into_inner(),
            evaluated_values: Some(evaluated_values.into()),
            name,
            subscripts,
            parameters: parameters.into_iter().collect(),
            description,
            used_decision_variable_ids: used_decision_variable_ids
                .into_iter()
                .map(|id| id.into_inner())
                .collect(),
        }
    }
}

impl Parse for v1::SampledNamedFunction {
    type Output = SampledNamedFunction;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.SampledNamedFunction";
        let evaluated_values = self
            .evaluated_values
            .ok_or(RawParseError::MissingField {
                message,
                field: "evaluated_values",
            })?
            .parse_as(&(), message, "evaluated_values")?;
        Ok(SampledNamedFunction {
            id: NamedFunctionID(self.id),
            evaluated_values,
            name: self.name,
            subscripts: self.subscripts,
            parameters: self.parameters.into_iter().collect(),
            description: self.description,
            used_decision_variable_ids: self
                .used_decision_variable_ids
                .into_iter()
                .map(VariableID::from)
                .collect(),
        })
    }
}
