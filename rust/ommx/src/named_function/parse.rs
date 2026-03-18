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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse::Parse, v1, VariableID};
    use maplit::btreeset;

    #[test]
    fn test_parse_named_function_missing_function() {
        // NamedFunction with function: None should error
        let nf = v1::NamedFunction {
            id: 1,
            function: Some(v1::Function { function: None }),
            name: Some("f".to_string()),
            subscripts: vec![],
            parameters: Default::default(),
            description: None,
        };
        let result: Result<NamedFunction, _> = nf.parse(&());
        insta::assert_snapshot!(result.unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.NamedFunction[function]
        Unsupported ommx.v1.Function is found. It is created by a newer version of OMMX SDK.
        "###);
    }

    #[test]
    fn test_parse_named_functions_duplicate_ids() {
        // Two NamedFunctions with the same ID should produce DuplicatedNamedFunctionID error
        let nfs = vec![
            v1::NamedFunction {
                id: 1,
                function: Some(v1::Function {
                    function: Some(v1::function::Function::Constant(5.0)),
                }),
                name: Some("f".to_string()),
                ..Default::default()
            },
            v1::NamedFunction {
                id: 1,
                function: Some(v1::Function {
                    function: Some(v1::function::Function::Constant(10.0)),
                }),
                name: Some("g".to_string()),
                ..Default::default()
            },
        ];
        let result: Result<BTreeMap<NamedFunctionID, NamedFunction>, _> = nfs.parse(&());
        insta::assert_snapshot!(result.unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        Duplicated named function ID is found in definition: NamedFunctionID(1)
        "###);
    }

    #[test]
    fn test_parse_evaluated_named_function() {
        // Parse EvaluatedNamedFunction with full metadata
        let v1_enf = v1::EvaluatedNamedFunction {
            id: 42,
            evaluated_value: 3.14,
            used_decision_variable_ids: vec![1, 2, 3],
            name: Some("objective_penalty".to_string()),
            subscripts: vec![10, 20],
            parameters: [("key1".to_string(), "value1".to_string())]
                .iter()
                .cloned()
                .collect(),
            description: Some("A test named function".to_string()),
        };

        let parsed: EvaluatedNamedFunction = v1_enf.parse(&()).unwrap();

        assert_eq!(parsed.id(), NamedFunctionID::from(42));
        assert_eq!(parsed.evaluated_value(), 3.14);
        assert_eq!(
            *parsed.used_decision_variable_ids(),
            btreeset! { VariableID::from(1), VariableID::from(2), VariableID::from(3) }
        );
        assert_eq!(*parsed.name(), Some("objective_penalty".to_string()));
        assert_eq!(*parsed.subscripts(), vec![10, 20]);
        assert_eq!(
            *parsed.description(),
            Some("A test named function".to_string())
        );
        assert!(parsed.parameters().contains_key("key1"));
        assert_eq!(parsed.parameters()["key1"], "value1");
    }

    #[test]
    fn test_parse_sampled_named_function() {
        // Parse SampledNamedFunction with full metadata and round-trip test
        let v1_snf = v1::SampledNamedFunction {
            id: 7,
            evaluated_values: Some(v1::SampledValues {
                entries: vec![
                    v1::sampled_values::SampledValuesEntry {
                        ids: vec![0, 1],
                        value: 1.5,
                    },
                    v1::sampled_values::SampledValuesEntry {
                        ids: vec![2],
                        value: 2.5,
                    },
                ],
            }),
            name: Some("cost".to_string()),
            subscripts: vec![1, 2],
            parameters: [("p".to_string(), "v".to_string())]
                .iter()
                .cloned()
                .collect(),
            description: Some("A sampled function".to_string()),
            used_decision_variable_ids: vec![10, 20],
        };

        let parsed: SampledNamedFunction = v1_snf.parse(&()).unwrap();

        assert_eq!(*parsed.id(), NamedFunctionID::from(7));
        assert_eq!(parsed.name, Some("cost".to_string()));
        assert_eq!(parsed.subscripts, vec![1, 2]);
        assert_eq!(parsed.description, Some("A sampled function".to_string()));
        assert!(parsed.parameters.contains_key("p"));
        assert_eq!(
            *parsed.used_decision_variable_ids(),
            btreeset! { VariableID::from(10), VariableID::from(20) }
        );

        // Round-trip: SampledNamedFunction -> v1::SampledNamedFunction
        let v1_converted: v1::SampledNamedFunction = parsed.into();
        assert_eq!(v1_converted.id, 7);
        assert_eq!(v1_converted.name, Some("cost".to_string()));
        assert!(v1_converted.evaluated_values.is_some());
    }

    #[test]
    fn test_parse_sampled_named_function_missing_evaluated_values() {
        // Missing evaluated_values should produce an error
        let v1_snf = v1::SampledNamedFunction {
            id: 1,
            evaluated_values: None,
            name: Some("f".to_string()),
            ..Default::default()
        };
        let result: Result<SampledNamedFunction, _> = v1_snf.parse(&());
        insta::assert_snapshot!(result.unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        Field evaluated_values in ommx.v1.SampledNamedFunction is missing.
        "###);
    }
}
