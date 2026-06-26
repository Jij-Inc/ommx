use super::*;

use crate::{parse::*, v1};
use std::collections::BTreeMap;

impl Parse for v1::decision_variable::Kind {
    type Output = Kind;
    type Context = ();
    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        use v1::decision_variable::Kind::*;
        match self {
            Continuous => Ok(Kind::Continuous),
            Integer => Ok(Kind::Integer),
            Binary => Ok(Kind::Binary),
            SemiContinuous => Ok(Kind::SemiContinuous),
            SemiInteger => Ok(Kind::SemiInteger),
            _ => Err(RawParseError::UnknownEnumValue {
                enum_name: "ommx.v1.decision_variable.Kind",
                value: self as i32,
            }
            .into()),
        }
    }
}

impl TryFrom<v1::decision_variable::Kind> for Kind {
    type Error = ParseError;
    fn try_from(value: v1::decision_variable::Kind) -> Result<Self, Self::Error> {
        value.parse(&())
    }
}

impl From<Kind> for v1::decision_variable::Kind {
    fn from(kind: Kind) -> Self {
        use v1::decision_variable::Kind::*;
        match kind {
            Kind::Continuous => Continuous,
            Kind::Integer => Integer,
            Kind::Binary => Binary,
            Kind::SemiContinuous => SemiContinuous,
            Kind::SemiInteger => SemiInteger,
        }
    }
}

impl From<Kind> for i32 {
    fn from(kind: Kind) -> Self {
        v1::decision_variable::Kind::from(kind) as i32
    }
}

/// Parsed v1 `DecisionVariable` together with its drained modeling label.
///
/// Per-element parse no longer attaches a label to the [`DecisionVariable`]
/// itself — the label is returned alongside so the collection-level
/// parse can drain it into the [`VariableLabelStore`].
#[derive(Debug)]
pub struct ParsedDecisionVariable {
    pub id: VariableID,
    pub variable: DecisionVariable,
    pub label: DecisionVariableLabel,
    pub fixed_value: Option<f64>,
}

impl Parse for v1::DecisionVariable {
    type Output = ParsedDecisionVariable;
    type Context = ();
    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.DecisionVariable";
        let kind = self.kind().parse_as(&(), message, "kind")?;
        let bound = self
            .bound
            .unwrap_or_default()
            .parse_as(&(), message, "bound")?;
        let id = VariableID::from(self.id);
        let dv = DecisionVariable::new(kind, bound, ATol::default()) // FIXME: user should provide this
            .map_err(|source| {
                RawParseError::InvalidDecisionVariable(DecisionVariableError::InvalidDefinition {
                    id,
                    source: Box::new(source),
                })
                .context(message, "bound")
            })?;
        let fixed_value = self.substituted_value;
        if let Some(value) = fixed_value {
            dv.check_value_consistency(id, value, ATol::default())
                .map_err(|e| {
                    RawParseError::InvalidDecisionVariable(e).context(message, "substituted_value")
                })?;
        }
        let label = DecisionVariableLabel {
            name: self.name,
            subscripts: self.subscripts,
            parameters: self.parameters.into_iter().collect(),
            description: self.description,
        };
        Ok(ParsedDecisionVariable {
            id,
            variable: dv,
            label,
            fixed_value,
        })
    }
}

impl TryFrom<v1::DecisionVariable> for DecisionVariable {
    type Error = ParseError;
    fn try_from(value: v1::DecisionVariable) -> Result<Self, Self::Error> {
        value.parse(&()).map(|p| p.variable)
    }
}

impl Parse for Vec<v1::DecisionVariable> {
    type Output = (
        BTreeMap<VariableID, DecisionVariable>,
        crate::VariableLabelStore,
        BTreeMap<VariableID, f64>,
    );
    type Context = ();
    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let mut decision_variables = BTreeMap::default();
        let mut label_store = crate::VariableLabelStore::default();
        let mut fixed_values = BTreeMap::default();
        for v in self {
            let parsed: ParsedDecisionVariable = v.parse(&())?;
            let id = parsed.id;
            if decision_variables.insert(id, parsed.variable).is_some() {
                return Err(RawParseError::InvalidInstance(format!(
                    "Duplicated variable ID is found in definition: {id:?}"
                ))
                .into());
            }
            if let Some(value) = parsed.fixed_value {
                fixed_values.insert(id, value);
            }
            label_store.insert(id, parsed.label);
        }
        Ok((decision_variables, label_store, fixed_values))
    }
}

/// Build a v1 `DecisionVariable` from its intrinsic data plus drained modeling label.
pub(crate) fn decision_variable_to_v1(
    id: VariableID,
    DecisionVariable { kind, bound }: DecisionVariable,
    label: DecisionVariableLabel,
) -> v1::DecisionVariable {
    decision_variable_fields_to_v1(id, kind, bound, label, None)
}

/// Build a v1 `DecisionVariable` and overlay the root-owned fixed value.
pub(crate) fn decision_variable_to_v1_with_fixed_value(
    id: VariableID,
    DecisionVariable { kind, bound }: DecisionVariable,
    label: DecisionVariableLabel,
    substituted_value: Option<f64>,
) -> v1::DecisionVariable {
    decision_variable_fields_to_v1(id, kind, bound, label, substituted_value)
}

fn decision_variable_fields_to_v1(
    id: VariableID,
    kind: Kind,
    bound: Bound,
    label: DecisionVariableLabel,
    substituted_value: Option<f64>,
) -> v1::DecisionVariable {
    v1::DecisionVariable {
        id: id.into_inner(),
        kind: kind.into(),
        bound: Some(bound.into()),
        substituted_value,
        name: label.name,
        subscripts: label.subscripts,
        parameters: label.parameters.into_iter().collect(),
        description: label.description,
    }
}

/// Parsed v1 `SampledDecisionVariable` together with its drained modeling label.
#[derive(Debug)]
pub struct ParsedSampledDecisionVariable {
    pub id: VariableID,
    pub variable: SampledDecisionVariable,
    pub label: DecisionVariableLabel,
}

impl Parse for v1::SampledDecisionVariable {
    type Output = ParsedSampledDecisionVariable;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.SampledDecisionVariable";

        // Parse the DecisionVariable
        let parsed_dv: ParsedDecisionVariable = self
            .decision_variable
            .ok_or(RawParseError::MissingField {
                message,
                field: "decision_variable",
            })?
            .parse_as(&(), message, "decision_variable")?;

        // Parse the samples
        let samples: crate::Sampled<f64> = self
            .samples
            .ok_or(RawParseError::MissingField {
                message,
                field: "samples",
            })?
            .parse_as(&(), message, "samples")?;

        if let Some(fixed_value) = parsed_dv.fixed_value {
            let atol = ATol::default();
            for (_, &sample_value) in samples.iter() {
                if !sample_value.is_finite() {
                    return Err(RawParseError::InvalidDecisionVariable(
                        DecisionVariableError::NonFiniteValue {
                            id: parsed_dv.id,
                            value: sample_value,
                        },
                    )
                    .context(message, "samples"));
                }
                if (sample_value - fixed_value).abs() > *atol {
                    return Err(RawParseError::InvalidDecisionVariable(
                        DecisionVariableError::SubstitutedValueOverwrite {
                            id: parsed_dv.id,
                            previous_value: fixed_value,
                            new_value: sample_value,
                            atol,
                        },
                    )
                    .context(message, "decision_variable"));
                }
            }
        }

        // Create SampledDecisionVariable with validation
        let sampled =
            crate::SampledDecisionVariable::new(parsed_dv.id, parsed_dv.variable, samples)
                .map_err(RawParseError::InvalidDecisionVariable)?;
        Ok(ParsedSampledDecisionVariable {
            id: parsed_dv.id,
            variable: sampled,
            label: parsed_dv.label,
        })
    }
}

impl TryFrom<v1::SampledDecisionVariable> for SampledDecisionVariable {
    type Error = ParseError;
    fn try_from(value: v1::SampledDecisionVariable) -> Result<Self, Self::Error> {
        value.parse(&()).map(|p| p.variable)
    }
}

/// Build a v1 `SampledDecisionVariable` from its intrinsic data plus drained modeling label.
pub(crate) fn sampled_decision_variable_to_v1(
    id: VariableID,
    sampled_dv: SampledDecisionVariable,
    label: DecisionVariableLabel,
) -> v1::SampledDecisionVariable {
    let dv = DecisionVariable {
        kind: sampled_dv.kind,
        bound: sampled_dv.bound,
    };

    v1::SampledDecisionVariable {
        decision_variable: Some(decision_variable_to_v1(id, dv, label)),
        samples: Some(sampled_dv.samples.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_decision_variable() {
        let dv = v1::DecisionVariable {
            id: 1,
            kind: v1::decision_variable::Kind::Integer as i32,
            bound: Some(v1::Bound {
                lower: 1.1,
                upper: 1.9,
            }),
            ..Default::default()
        };
        let res: Result<ParsedDecisionVariable, _> = dv.parse(&());
        insta::assert_snapshot!(res.unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.DecisionVariable[bound]
        Invalid decision variable ID=1: Bound is inconsistent to kind: kind=Integer, bound=[1.1, 1.9]
        "###);
    }

    #[test]
    fn test_parse_sampled_decision_variable() {
        let v1_sampled_dv = v1::SampledDecisionVariable {
            decision_variable: Some(v1::DecisionVariable {
                id: 42,
                kind: v1::decision_variable::Kind::Continuous as i32,
                bound: Some(v1::Bound {
                    lower: 0.0,
                    upper: 10.0,
                }),
                name: Some("test_var".to_string()),
                subscripts: vec![1, 2],
                parameters: vec![("param1".to_string(), "value1".to_string())]
                    .into_iter()
                    .collect(),
                description: Some("A test variable".to_string()),
                ..Default::default()
            }),
            samples: Some(v1::SampledValues {
                entries: vec![
                    v1::sampled_values::SampledValuesEntry {
                        ids: vec![0, 1],
                        value: 1.0,
                    },
                    v1::sampled_values::SampledValuesEntry {
                        ids: vec![2],
                        value: 2.0,
                    },
                ],
            }),
        };

        let parsed: ParsedSampledDecisionVariable = v1_sampled_dv.parse(&()).unwrap();
        let sampled_id = parsed.id;
        let sampled_dv = parsed.variable;
        let label = parsed.label;

        assert_eq!(sampled_id, VariableID::from(42));
        assert_eq!(*sampled_dv.kind(), Kind::Continuous);
        assert_eq!(label.name, Some("test_var".to_string()));
        assert_eq!(label.subscripts, vec![1, 2]);
        assert_eq!(label.description, Some("A test variable".to_string()));

        // Test round-trip conversion: name is reattached at serialize time
        // by `sampled_decision_variable_to_v1`.
        let v1_converted = sampled_decision_variable_to_v1(sampled_id, sampled_dv, label);
        let decision_variable = v1_converted.decision_variable.unwrap();
        assert_eq!(decision_variable.id, 42);
        assert_eq!(decision_variable.name, Some("test_var".to_string()));
    }

    #[test]
    fn test_parse_sampled_decision_variable_rejects_inconsistent_substituted_value() {
        let v1_sampled_dv = v1::SampledDecisionVariable {
            decision_variable: Some(v1::DecisionVariable {
                id: 42,
                kind: v1::decision_variable::Kind::Continuous as i32,
                bound: Some(v1::Bound {
                    lower: 0.0,
                    upper: 10.0,
                }),
                substituted_value: Some(1.0),
                ..Default::default()
            }),
            samples: Some(v1::SampledValues {
                entries: vec![v1::sampled_values::SampledValuesEntry {
                    ids: vec![0],
                    value: 2.0,
                }],
            }),
        };

        let result: Result<ParsedSampledDecisionVariable, _> = v1_sampled_dv.parse(&());
        insta::assert_snapshot!(result.unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.SampledDecisionVariable[decision_variable]
        Substituted value for ID=42 cannot be overwritten: previous=1, new=2, atol=ATol(1e-6)
        "###);
    }

    #[test]
    fn test_parse_sampled_decision_variable_accepts_substituted_value_at_atol_boundary() {
        let atol = *ATol::default();
        let v1_sampled_dv = v1::SampledDecisionVariable {
            decision_variable: Some(v1::DecisionVariable {
                id: 42,
                kind: v1::decision_variable::Kind::Continuous as i32,
                bound: Some(v1::Bound {
                    lower: 0.0,
                    upper: 10.0,
                }),
                substituted_value: Some(0.0),
                ..Default::default()
            }),
            samples: Some(v1::SampledValues {
                entries: vec![v1::sampled_values::SampledValuesEntry {
                    ids: vec![0],
                    value: atol,
                }],
            }),
        };

        let parsed: ParsedSampledDecisionVariable = v1_sampled_dv.parse(&()).unwrap();
        assert_eq!(
            *parsed
                .variable
                .samples()
                .get(crate::SampleID::from(0))
                .unwrap(),
            atol
        );
    }

    #[test]
    fn test_parse_sampled_decision_variable_rejects_non_finite_sample_with_substituted_value() {
        let v1_sampled_dv = v1::SampledDecisionVariable {
            decision_variable: Some(v1::DecisionVariable {
                id: 42,
                kind: v1::decision_variable::Kind::Continuous as i32,
                bound: Some(v1::Bound {
                    lower: 0.0,
                    upper: 10.0,
                }),
                substituted_value: Some(1.0),
                ..Default::default()
            }),
            samples: Some(v1::SampledValues {
                entries: vec![v1::sampled_values::SampledValuesEntry {
                    ids: vec![0],
                    value: f64::NAN,
                }],
            }),
        };

        let result: Result<ParsedSampledDecisionVariable, _> = v1_sampled_dv.parse(&());
        insta::assert_snapshot!(result.unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.SampledDecisionVariable[samples]
        Decision variable value for ID=42 must be finite: value=NaN
        "###);
    }

    #[test]
    fn test_parse_sampled_decision_variable_missing_decision_variable() {
        let v1_sampled_dv = v1::SampledDecisionVariable {
            decision_variable: None, // Missing decision variable should cause error
            samples: Some(v1::SampledValues {
                entries: vec![v1::sampled_values::SampledValuesEntry {
                    ids: vec![0],
                    value: 1.0,
                }],
            }),
        };

        let result: Result<ParsedSampledDecisionVariable, _> = v1_sampled_dv.parse(&());
        insta::assert_snapshot!(result.unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        Field decision_variable in ommx.v1.SampledDecisionVariable is missing.
        "###);
    }

    #[test]
    fn test_parse_sampled_decision_variable_missing_samples() {
        let v1_sampled_dv = v1::SampledDecisionVariable {
            decision_variable: Some(v1::DecisionVariable {
                id: 1,
                kind: v1::decision_variable::Kind::Continuous as i32,
                ..Default::default()
            }),
            samples: None, // Missing samples should cause error
        };

        let result: Result<ParsedSampledDecisionVariable, _> = v1_sampled_dv.parse(&());
        insta::assert_snapshot!(result.unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        Field samples in ommx.v1.SampledDecisionVariable is missing.
        "###);
    }
}
