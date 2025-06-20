use super::*;

use crate::{parse::*, v1, InstanceError};
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

impl Parse for v1::DecisionVariable {
    type Output = DecisionVariable;
    type Context = ();
    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.DecisionVariable";
        let kind = self.kind().parse_as(&(), message, "kind")?;
        let bound = self
            .bound
            .unwrap_or_default()
            .parse_as(&(), message, "bound")?;
        let mut dv = DecisionVariable::new(
            VariableID(self.id),
            kind,
            bound,
            self.substituted_value,
            ATol::default(), // FIXME: user should provide this
        )
        .map_err(|e| RawParseError::InvalidDecisionVariable(e).context(message, "bound"))?;
        dv.metadata.name = self.name;
        dv.metadata.subscripts = self.subscripts;
        dv.metadata.parameters = self.parameters.into_iter().collect();
        dv.metadata.description = self.description;
        Ok(dv)
    }
}

impl TryFrom<v1::DecisionVariable> for DecisionVariable {
    type Error = ParseError;
    fn try_from(value: v1::DecisionVariable) -> Result<Self, Self::Error> {
        value.parse(&())
    }
}

impl Parse for Vec<v1::DecisionVariable> {
    type Output = BTreeMap<VariableID, DecisionVariable>;
    type Context = ();
    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let mut decision_variables = BTreeMap::default();
        for v in self {
            let v: DecisionVariable = v.parse(&())?;
            let id = v.id;
            if decision_variables.insert(id, v).is_some() {
                return Err(
                    RawParseError::InstanceError(InstanceError::DuplicatedVariableID { id }).into(),
                );
            }
        }
        Ok(decision_variables)
    }
}

impl From<DecisionVariable> for v1::DecisionVariable {
    fn from(
        DecisionVariable {
            id,
            kind,
            bound,
            substituted_value,
            metadata,
        }: DecisionVariable,
    ) -> Self {
        Self {
            id: id.into_inner(),
            kind: kind.into(),
            bound: Some(bound.into()),
            substituted_value,
            name: metadata.name,
            subscripts: metadata.subscripts,
            parameters: metadata.parameters.into_iter().collect(),
            description: metadata.description,
        }
    }
}

impl Parse for v1::SampledDecisionVariable {
    type Output = SampledDecisionVariable;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.SampledDecisionVariable";

        // Parse the DecisionVariable
        let dv = self
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

        // Create SampledDecisionVariable with validation
        crate::SampledDecisionVariable::new(dv, samples, crate::ATol::default())
            .map_err(|e| RawParseError::InvalidDecisionVariable(e).into())
    }
}

impl TryFrom<v1::SampledDecisionVariable> for SampledDecisionVariable {
    type Error = ParseError;
    fn try_from(value: v1::SampledDecisionVariable) -> Result<Self, Self::Error> {
        value.parse(&())
    }
}

impl From<SampledDecisionVariable> for v1::SampledDecisionVariable {
    fn from(sampled_dv: SampledDecisionVariable) -> Self {
        // Convert back to DecisionVariable
        let dv = DecisionVariable {
            id: sampled_dv.id,
            kind: sampled_dv.kind,
            bound: sampled_dv.bound,
            substituted_value: None, // SampledDecisionVariable doesn't have substituted_value
            metadata: sampled_dv.metadata,
        };

        Self {
            decision_variable: Some(dv.into()),
            samples: Some(sampled_dv.samples.into()),
        }
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
        insta::assert_snapshot!(dv.parse(&()).unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.DecisionVariable[bound]
        Bound for ID=1 is inconsistent to kind: kind=Integer, bound=[1.1, 1.9]
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

        let sampled_dv: SampledDecisionVariable = v1_sampled_dv.parse(&()).unwrap();

        assert_eq!(*sampled_dv.id(), VariableID::from(42));
        assert_eq!(*sampled_dv.kind(), Kind::Continuous);
        assert_eq!(sampled_dv.metadata.name, Some("test_var".to_string()));
        assert_eq!(sampled_dv.metadata.subscripts, vec![1, 2]);
        assert_eq!(
            sampled_dv.metadata.description,
            Some("A test variable".to_string())
        );

        // Test round-trip conversion
        let v1_converted: v1::SampledDecisionVariable = sampled_dv.into();
        let decision_variable = v1_converted.decision_variable.unwrap();
        assert_eq!(decision_variable.id, 42);
        assert_eq!(decision_variable.name, Some("test_var".to_string()));
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

        let result: Result<SampledDecisionVariable, _> = v1_sampled_dv.parse(&());
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

        let result: Result<SampledDecisionVariable, _> = v1_sampled_dv.parse(&());
        insta::assert_snapshot!(result.unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        Field samples in ommx.v1.SampledDecisionVariable is missing.
        "###);
    }
}
