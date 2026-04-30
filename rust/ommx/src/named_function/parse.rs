use std::collections::BTreeMap;

use super::*;
use crate::{
    parse::{Parse, ParseError, RawParseError},
    v1, VariableID,
};
use anyhow::Result;

/// Parsed v1 `NamedFunction` together with its drained metadata.
///
/// Per-element parse no longer attaches metadata to the [`NamedFunction`]
/// itself — the metadata is returned alongside so the collection-level
/// parse can drain it into the [`NamedFunctionMetadataStore`].
#[derive(Debug)]
pub struct ParsedNamedFunction {
    pub named_function: NamedFunction,
    pub metadata: NamedFunctionMetadata,
}

impl Parse for v1::NamedFunction {
    type Output = ParsedNamedFunction;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.NamedFunction";
        let function = self
            .function
            .ok_or(RawParseError::MissingField {
                message,
                field: "function",
            })?
            .parse_as(&(), message, "function")?;
        let named_function = NamedFunction {
            id: NamedFunctionID(self.id),
            function,
        };
        let metadata = NamedFunctionMetadata {
            name: self.name,
            subscripts: self.subscripts,
            parameters: self.parameters.into_iter().collect(),
            description: self.description,
        };
        Ok(ParsedNamedFunction {
            named_function,
            metadata,
        })
    }
}

impl Parse for Vec<v1::NamedFunction> {
    type Output = (
        BTreeMap<NamedFunctionID, NamedFunction>,
        NamedFunctionMetadataStore,
    );
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let mut named_functions = BTreeMap::new();
        let mut metadata_store = NamedFunctionMetadataStore::default();
        for v in self {
            let parsed: ParsedNamedFunction = v.parse(&())?;
            let id = parsed.named_function.id;
            if named_functions.insert(id, parsed.named_function).is_some() {
                return Err(RawParseError::InvalidInstance(format!(
                    "Duplicated named function ID is found in definition: {id:?}"
                ))
                .into());
            }
            metadata_store.insert(id, parsed.metadata);
        }
        Ok((named_functions, metadata_store))
    }
}

/// Build a v1 `NamedFunction` from its intrinsic data plus drained metadata.
pub(crate) fn named_function_to_v1(
    NamedFunction { id, function }: NamedFunction,
    metadata: NamedFunctionMetadata,
) -> v1::NamedFunction {
    v1::NamedFunction {
        id: id.into_inner(),
        function: Some(function.into()),
        name: metadata.name,
        subscripts: metadata.subscripts,
        parameters: metadata.parameters.into_iter().collect(),
        description: metadata.description,
    }
}

/// Parsed v1 `EvaluatedNamedFunction` together with its drained metadata.
#[derive(Debug)]
pub struct ParsedEvaluatedNamedFunction {
    pub evaluated_named_function: EvaluatedNamedFunction,
    pub metadata: NamedFunctionMetadata,
}

impl Parse for v1::EvaluatedNamedFunction {
    type Output = ParsedEvaluatedNamedFunction;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let evaluated_named_function = EvaluatedNamedFunction {
            id: NamedFunctionID(self.id),
            evaluated_value: self.evaluated_value,
            used_decision_variable_ids: self
                .used_decision_variable_ids
                .into_iter()
                .map(VariableID::from)
                .collect(),
        };
        let metadata = NamedFunctionMetadata {
            name: self.name,
            subscripts: self.subscripts,
            parameters: self.parameters.into_iter().collect(),
            description: self.description,
        };
        Ok(ParsedEvaluatedNamedFunction {
            evaluated_named_function,
            metadata,
        })
    }
}

/// Build a v1 `EvaluatedNamedFunction` from its intrinsic data plus drained metadata.
pub(crate) fn evaluated_named_function_to_v1(
    EvaluatedNamedFunction {
        id,
        evaluated_value,
        used_decision_variable_ids,
    }: EvaluatedNamedFunction,
    metadata: NamedFunctionMetadata,
) -> v1::EvaluatedNamedFunction {
    v1::EvaluatedNamedFunction {
        id: id.into_inner(),
        evaluated_value,
        name: metadata.name,
        subscripts: metadata.subscripts,
        parameters: metadata.parameters.into_iter().collect(),
        description: metadata.description,
        used_decision_variable_ids: used_decision_variable_ids
            .into_iter()
            .map(|id| id.into_inner())
            .collect(),
    }
}

/// Parsed v1 `SampledNamedFunction` together with its drained metadata.
#[derive(Debug)]
pub struct ParsedSampledNamedFunction {
    pub sampled_named_function: SampledNamedFunction,
    pub metadata: NamedFunctionMetadata,
}

impl Parse for v1::SampledNamedFunction {
    type Output = ParsedSampledNamedFunction;
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
        let sampled_named_function = SampledNamedFunction {
            id: NamedFunctionID(self.id),
            evaluated_values,
            used_decision_variable_ids: self
                .used_decision_variable_ids
                .into_iter()
                .map(VariableID::from)
                .collect(),
        };
        let metadata = NamedFunctionMetadata {
            name: self.name,
            subscripts: self.subscripts,
            parameters: self.parameters.into_iter().collect(),
            description: self.description,
        };
        Ok(ParsedSampledNamedFunction {
            sampled_named_function,
            metadata,
        })
    }
}

/// Build a v1 `SampledNamedFunction` from its intrinsic data plus drained metadata.
pub(crate) fn sampled_named_function_to_v1(
    sampled: SampledNamedFunction,
    metadata: NamedFunctionMetadata,
) -> v1::SampledNamedFunction {
    let SampledNamedFunction {
        id,
        evaluated_values,
        used_decision_variable_ids,
    } = sampled;
    v1::SampledNamedFunction {
        id: id.into_inner(),
        evaluated_values: Some(evaluated_values.into()),
        name: metadata.name,
        subscripts: metadata.subscripts,
        parameters: metadata.parameters.into_iter().collect(),
        description: metadata.description,
        used_decision_variable_ids: used_decision_variable_ids
            .into_iter()
            .map(|id| id.into_inner())
            .collect(),
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
        let result: Result<ParsedNamedFunction, _> = nf.parse(&());
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
        let result: Result<(BTreeMap<NamedFunctionID, NamedFunction>, _), _> = nfs.parse(&());
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

        let parsed: ParsedEvaluatedNamedFunction = v1_enf.parse(&()).unwrap();
        let enf = parsed.evaluated_named_function;
        let metadata = parsed.metadata;

        assert_eq!(enf.id(), NamedFunctionID::from(42));
        assert_eq!(enf.evaluated_value(), 3.14);
        assert_eq!(
            *enf.used_decision_variable_ids(),
            btreeset! { VariableID::from(1), VariableID::from(2), VariableID::from(3) }
        );
        assert_eq!(metadata.name, Some("objective_penalty".to_string()));
        assert_eq!(metadata.subscripts, vec![10, 20]);
        assert_eq!(
            metadata.description,
            Some("A test named function".to_string())
        );
        assert!(metadata.parameters.contains_key("key1"));
        assert_eq!(metadata.parameters["key1"], "value1");
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

        let parsed: ParsedSampledNamedFunction = v1_snf.parse(&()).unwrap();
        let snf = parsed.sampled_named_function;
        let metadata = parsed.metadata;

        assert_eq!(*snf.id(), NamedFunctionID::from(7));
        assert_eq!(metadata.name, Some("cost".to_string()));
        assert_eq!(metadata.subscripts, vec![1, 2]);
        assert_eq!(metadata.description, Some("A sampled function".to_string()));
        assert!(metadata.parameters.contains_key("p"));
        assert_eq!(
            *snf.used_decision_variable_ids(),
            btreeset! { VariableID::from(10), VariableID::from(20) }
        );

        // Round-trip: SampledNamedFunction + metadata -> v1::SampledNamedFunction
        let v1_converted = sampled_named_function_to_v1(snf, metadata);
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
        let result: Result<ParsedSampledNamedFunction, _> = v1_snf.parse(&());
        insta::assert_snapshot!(result.unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        Field evaluated_values in ommx.v1.SampledNamedFunction is missing.
        "###);
    }
}
