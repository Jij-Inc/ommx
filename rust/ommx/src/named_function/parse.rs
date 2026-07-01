use std::collections::BTreeMap;

use super::*;
use crate::{
    parse::{Parse, ParseError, RawParseError},
    v1, VariableID,
};
use anyhow::Result;

/// Parsed v1 `NamedFunction` together with its drained modeling label.
///
/// Per-element parse no longer attaches a label to the [`NamedFunction`]
/// itself — the label is returned alongside so the collection-level
/// parse can drain it into the [`NamedFunctionLabelStore`].
#[derive(Debug)]
pub struct ParsedNamedFunction {
    pub id: NamedFunctionID,
    pub named_function: NamedFunction,
    pub label: NamedFunctionLabel,
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
        let id = NamedFunctionID(self.id);
        let named_function = NamedFunction { function };
        let label = NamedFunctionLabel {
            name: self.name,
            subscripts: self.subscripts,
            parameters: self.parameters.into_iter().collect(),
            description: self.description,
        };
        Ok(ParsedNamedFunction {
            id,
            named_function,
            label,
        })
    }
}

impl Parse for Vec<v1::NamedFunction> {
    type Output = (
        BTreeMap<NamedFunctionID, NamedFunction>,
        NamedFunctionLabelStore,
    );
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let mut named_functions = BTreeMap::new();
        let mut label_store = NamedFunctionLabelStore::default();
        for v in self {
            let parsed: ParsedNamedFunction = v.parse(&())?;
            let id = parsed.id;
            if named_functions.insert(id, parsed.named_function).is_some() {
                return Err(RawParseError::InvalidInstance(format!(
                    "Duplicated named function ID is found in definition: {id:?}"
                ))
                .into());
            }
            label_store.insert(id, parsed.label);
        }
        Ok((named_functions, label_store))
    }
}

/// Parsed v1 `EvaluatedNamedFunction` together with its drained modeling label.
#[derive(Debug)]
pub struct ParsedEvaluatedNamedFunction {
    pub id: NamedFunctionID,
    pub evaluated_named_function: EvaluatedNamedFunction,
    pub label: NamedFunctionLabel,
}

impl Parse for v1::EvaluatedNamedFunction {
    type Output = ParsedEvaluatedNamedFunction;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let id = NamedFunctionID(self.id);
        let evaluated_named_function = EvaluatedNamedFunction {
            evaluated_value: self.evaluated_value,
            used_decision_variable_ids: self
                .used_decision_variable_ids
                .into_iter()
                .map(VariableID::from)
                .collect(),
        };
        let label = NamedFunctionLabel {
            name: self.name,
            subscripts: self.subscripts,
            parameters: self.parameters.into_iter().collect(),
            description: self.description,
        };
        Ok(ParsedEvaluatedNamedFunction {
            id,
            evaluated_named_function,
            label,
        })
    }
}

/// Parsed v1 `SampledNamedFunction` together with its drained modeling label.
#[derive(Debug)]
pub struct ParsedSampledNamedFunction {
    pub id: NamedFunctionID,
    pub sampled_named_function: SampledNamedFunction,
    pub label: NamedFunctionLabel,
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
        let id = NamedFunctionID(self.id);
        let sampled_named_function = SampledNamedFunction {
            evaluated_values,
            used_decision_variable_ids: self
                .used_decision_variable_ids
                .into_iter()
                .map(VariableID::from)
                .collect(),
        };
        let label = NamedFunctionLabel {
            name: self.name,
            subscripts: self.subscripts,
            parameters: self.parameters.into_iter().collect(),
            description: self.description,
        };
        Ok(ParsedSampledNamedFunction {
            id,
            sampled_named_function,
            label,
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
        // Parse EvaluatedNamedFunction with a full modeling label
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
        let id = parsed.id;
        let enf = parsed.evaluated_named_function;
        let label = parsed.label;

        assert_eq!(id, NamedFunctionID::from(42));
        assert_eq!(enf.evaluated_value(), 3.14);
        assert_eq!(
            *enf.used_decision_variable_ids(),
            btreeset! { VariableID::from(1), VariableID::from(2), VariableID::from(3) }
        );
        assert_eq!(label.name, Some("objective_penalty".to_string()));
        assert_eq!(label.subscripts, vec![10, 20]);
        assert_eq!(label.description, Some("A test named function".to_string()));
        assert!(label.parameters.contains_key("key1"));
        assert_eq!(label.parameters["key1"], "value1");
    }

    #[test]
    fn test_parse_sampled_named_function() {
        // Parse SampledNamedFunction with a full modeling label and round-trip test
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
        let id = parsed.id;
        let snf = parsed.sampled_named_function;
        let label = parsed.label;

        assert_eq!(id, NamedFunctionID::from(7));
        assert_eq!(label.name, Some("cost".to_string()));
        assert_eq!(label.subscripts, vec![1, 2]);
        assert_eq!(label.description, Some("A sampled function".to_string()));
        assert!(label.parameters.contains_key("p"));
        assert_eq!(
            *snf.used_decision_variable_ids(),
            btreeset! { VariableID::from(10), VariableID::from(20) }
        );

        // Round-trip through the table, where labels live.
        let mut labels = NamedFunctionLabelStore::default();
        labels.insert(id, label);
        let table =
            NamedFunctionTable::new(std::collections::BTreeMap::from([(id, snf)]), labels).unwrap();
        let mut rows: Vec<v1::SampledNamedFunction> = table.into();
        let v1_converted = rows.pop().unwrap();
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
