use super::*;
use crate::{Parse, ParseError, RawParseError};

impl Parse for crate::v1::SampleSet {
    type Output = SampleSet;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let decision_variables = self.decision_variables;

        // Parse objectives if present
        let objectives = self.objectives.map(|obj| obj.parse(&())).transpose()?;

        // Parse constraints
        let constraints: Result<Vec<crate::SampledConstraint>, ParseError> = self
            .constraints
            .into_iter()
            .map(|sc| sc.parse(&()))
            .collect();
        let constraints = constraints?;

        let feasible_relaxed: FnvHashMap<u64, bool> = self.feasible_relaxed.into_iter().collect();
        let feasible: FnvHashMap<u64, bool> = self.feasible.into_iter().collect();
        let sense = self
            .sense
            .try_into()
            .map_err(|_| RawParseError::UnknownEnumValue {
                enum_name: "ommx.v1.Sense",
                value: self.sense,
            })?;

        Ok(SampleSet {
            decision_variables,
            objectives,
            constraints,
            feasible_relaxed,
            feasible,
            sense,
        })
    }
}

impl From<SampleSet> for crate::v1::SampleSet {
    fn from(sample_set: SampleSet) -> Self {
        let decision_variables = sample_set.decision_variables().clone();
        let objectives = sample_set
            .objectives()
            .as_ref()
            .map(|obj| obj.clone().into());
        let constraints = sample_set
            .constraints()
            .iter()
            .map(|sc| sc.clone().into())
            .collect();
        let feasible_relaxed: std::collections::HashMap<u64, bool> =
            sample_set.feasible_relaxed().clone().into_iter().collect();
        let feasible: std::collections::HashMap<u64, bool> =
            sample_set.feasible().clone().into_iter().collect();
        let sense = (*sample_set.sense()).into();

        crate::v1::SampleSet {
            decision_variables,
            objectives,
            constraints,
            feasible_relaxed,
            feasible,
            sense,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{v1, Parse};

    #[test]
    fn test_sample_set_parse() {
        let v1_sample_set = v1::SampleSet {
            decision_variables: vec![v1::SampledDecisionVariable {
                decision_variable: Some(v1::DecisionVariable {
                    id: 1,
                    name: Some("x1".to_string()),
                    kind: v1::decision_variable::Kind::Continuous as i32,
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
            }],
            objectives: Some(v1::SampledValues {
                entries: vec![
                    v1::sampled_values::SampledValuesEntry {
                        ids: vec![0, 1],
                        value: 10.0,
                    },
                    v1::sampled_values::SampledValuesEntry {
                        ids: vec![2],
                        value: 20.0,
                    },
                ],
            }),
            constraints: vec![],
            feasible_relaxed: [(0, true), (1, true), (2, false)].iter().cloned().collect(),
            feasible: [(0, true), (1, false), (2, false)]
                .iter()
                .cloned()
                .collect(),
            sense: v1::instance::Sense::Minimize as i32,
            ..Default::default()
        };

        let parsed: SampleSet = v1_sample_set.parse(&()).unwrap();

        assert_eq!(parsed.sense(), &crate::Sense::Minimize);
        assert_eq!(parsed.decision_variables().len(), 1);
        assert!(parsed.objectives().is_some());
        assert_eq!(parsed.constraints().len(), 0);

        // Test feasibility checks
        let sample_id_0 = crate::SampleID::from(0);
        let sample_id_1 = crate::SampleID::from(1);
        let sample_id_2 = crate::SampleID::from(2);

        assert_eq!(parsed.is_sample_feasible(sample_id_0), Some(true));
        assert_eq!(parsed.is_sample_feasible(sample_id_1), Some(false));
        assert_eq!(parsed.is_sample_feasible(sample_id_2), Some(false));

        assert_eq!(parsed.is_sample_feasible_relaxed(sample_id_0), Some(true));
        assert_eq!(parsed.is_sample_feasible_relaxed(sample_id_1), Some(true));
        assert_eq!(parsed.is_sample_feasible_relaxed(sample_id_2), Some(false));

        // Test round-trip conversion
        let v1_converted: v1::SampleSet = parsed.into();
        assert_eq!(v1_converted.sense, v1::instance::Sense::Minimize as i32);
        assert_eq!(v1_converted.decision_variables.len(), 1);
    }

    #[test]
    fn test_unknown_sense_enum_value() {
        // Test with an invalid sense value in SampleSet
        let v1_sample_set = v1::SampleSet {
            sense: 999, // Unknown enum value
            ..Default::default()
        };

        let result: Result<SampleSet, ParseError> = v1_sample_set.parse(&());
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error
            .to_string()
            .contains("Unknown or unsupported enum value 999 for ommx.v1.Sense"));
    }
}
