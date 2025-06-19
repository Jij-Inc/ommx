use super::*;
use crate::{Parse, ParseError, SampleSetError};

impl Parse for crate::v1::SampleSet {
    type Output = SampleSet;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        // Parse decision variables into BTreeMap
        let mut decision_variables = std::collections::BTreeMap::new();
        for v1_sampled_dv in self.decision_variables {
            // Parse the DecisionVariable
            let dv = v1_sampled_dv
                .decision_variable
                .ok_or(crate::RawParseError::MissingField {
                    message: "ommx.v1.SampledDecisionVariable",
                    field: "decision_variable",
                })?
                .parse(&())?;

            // Parse the samples
            let samples: crate::Sampled<f64> = v1_sampled_dv
                .samples
                .ok_or(crate::RawParseError::MissingField {
                    message: "ommx.v1.SampledDecisionVariable",
                    field: "samples",
                })?
                .try_into()
                .map_err(|_| crate::RawParseError::MissingField {
                    message: "ommx.v1.SampledDecisionVariable",
                    field: "samples",
                })?;

            // Create SampledDecisionVariable
            let dv_id = dv.id();
            let sampled_dv = crate::SampledDecisionVariable::new(
                dv,
                samples,
            );

            decision_variables.insert(dv_id, sampled_dv);
        }

        // Parse objectives - required, not optional
        let objectives = self
            .objectives
            .ok_or(crate::RawParseError::MissingField {
                message: "ommx.v1.SampleSet",
                field: "objectives",
            })?
            .parse(&())?;

        // Parse constraints into BTreeMap
        let mut constraints = std::collections::BTreeMap::new();
        for v1_constraint in self.constraints {
            let parsed_constraint: crate::SampledConstraint = v1_constraint.parse(&())?;
            constraints.insert(*parsed_constraint.id(), parsed_constraint);
        }

        let sense = self
            .sense
            .try_into()
            .map_err(|_| crate::RawParseError::UnknownEnumValue {
                enum_name: "ommx.v1.Sense",
                value: self.sense,
            })?;

        let provided_feasible_relaxed: BTreeMap<SampleID, bool> = self
            .feasible_relaxed
            .into_iter()
            .map(|(k, v)| (SampleID::from(k), v))
            .collect();
        let provided_feasible: BTreeMap<SampleID, bool> = self
            .feasible
            .into_iter()
            .map(|(k, v)| (SampleID::from(k), v))
            .collect();

        // Create SampleSet with validation
        let sample_set = SampleSet::new(decision_variables, objectives, constraints, sense)
            .map_err(crate::RawParseError::SampleSetError)?;

        // If constraints are present, validate feasibility consistency
        if !sample_set.constraints().is_empty() {
            for (sample_id, provided_feasible_value) in &provided_feasible {
                let computed_feasible =
                    sample_set.is_sample_feasible(*sample_id).map_err(|_| {
                        crate::RawParseError::SampleSetError(
                            SampleSetError::InconsistentSampleIDs {
                                context: "feasible validation".to_string(),
                                expected: sample_set.sample_ids(),
                                found: provided_feasible.keys().copied().collect(),
                            },
                        )
                    })?;
                if computed_feasible != *provided_feasible_value {
                    return Err(crate::RawParseError::SampleSetError(
                        SampleSetError::InconsistentFeasibility {
                            sample_id: sample_id.into_inner(),
                            provided_feasible: *provided_feasible_value,
                            computed_feasible,
                        },
                    )
                    .into());
                }
            }

            for (sample_id, provided_feasible_relaxed_value) in &provided_feasible_relaxed {
                let computed_feasible_relaxed = sample_set
                    .is_sample_feasible_relaxed(*sample_id)
                    .map_err(|_| {
                    crate::RawParseError::SampleSetError(SampleSetError::InconsistentSampleIDs {
                        context: "feasible_relaxed validation".to_string(),
                        expected: sample_set.sample_ids(),
                        found: provided_feasible_relaxed.keys().copied().collect(),
                    })
                })?;
                if computed_feasible_relaxed != *provided_feasible_relaxed_value {
                    return Err(crate::RawParseError::SampleSetError(
                        SampleSetError::InconsistentFeasibilityRelaxed {
                            sample_id: sample_id.into_inner(),
                            provided_feasible_relaxed: *provided_feasible_relaxed_value,
                            computed_feasible_relaxed,
                        },
                    )
                    .into());
                }
            }
        } else {
            // If no constraints, all samples are feasible (no validation needed)
            // The SampleSet::new already computed this correctly
        }

        Ok(sample_set)
    }
}

impl From<SampleSet> for crate::v1::SampleSet {
    fn from(sample_set: SampleSet) -> Self {
        let decision_variables: Vec<crate::v1::SampledDecisionVariable> = sample_set
            .decision_variables()
            .values()
            .map(|dv| dv.clone().into())
            .collect();
        let objectives = Some(sample_set.objectives().clone().into());
        let constraints: Vec<crate::v1::SampledConstraint> = sample_set
            .constraints()
            .values()
            .map(|sc| sc.clone().into())
            .collect();
        let sense = (*sample_set.sense()).into();

        // Compute feasible maps from constraint evaluations
        let mut feasible_relaxed = std::collections::HashMap::new();
        let mut feasible = std::collections::HashMap::new();

        // Get all sample IDs from objectives
        for (sample_id, _) in sample_set.objectives().iter() {
            let sample_id_u64 = sample_id.into_inner();
            // These should always succeed since we're iterating over known sample IDs
            let is_feasible = sample_set
                .is_sample_feasible(*sample_id)
                .expect("Sample ID should exist");
            let is_feasible_relaxed = sample_set
                .is_sample_feasible_relaxed(*sample_id)
                .expect("Sample ID should exist");
            feasible.insert(sample_id_u64, is_feasible);
            feasible_relaxed.insert(sample_id_u64, is_feasible_relaxed);
        }

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
        assert_eq!(parsed.constraints().len(), 0);

        // Test feasibility checks
        let sample_id_0 = crate::SampleID::from(0);
        let sample_id_1 = crate::SampleID::from(1);
        let sample_id_2 = crate::SampleID::from(2);

        // Since there are no constraints, all samples should be feasible
        assert!(parsed.is_sample_feasible(sample_id_0).unwrap());
        assert!(parsed.is_sample_feasible(sample_id_1).unwrap());
        assert!(parsed.is_sample_feasible(sample_id_2).unwrap());

        assert!(parsed.is_sample_feasible_relaxed(sample_id_0).unwrap());
        assert!(parsed.is_sample_feasible_relaxed(sample_id_1).unwrap());
        assert!(parsed.is_sample_feasible_relaxed(sample_id_2).unwrap());

        // Test error handling for unknown sample IDs
        let unknown_sample_id = crate::SampleID::from(999);
        assert!(parsed.is_sample_feasible(unknown_sample_id).is_err());
        assert!(parsed
            .is_sample_feasible_relaxed(unknown_sample_id)
            .is_err());

        // Test round-trip conversion
        let v1_converted: v1::SampleSet = parsed.into();
        assert_eq!(v1_converted.sense, v1::instance::Sense::Minimize as i32);
        assert_eq!(v1_converted.decision_variables.len(), 1);
    }

    #[test]
    fn test_unknown_sense_enum_value() {
        // Test with an invalid sense value in SampleSet
        let v1_sample_set = v1::SampleSet {
            objectives: Some(v1::SampledValues {
                entries: vec![v1::sampled_values::SampledValuesEntry {
                    ids: vec![0],
                    value: 10.0,
                }],
            }),
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

    #[test]
    fn test_inconsistent_feasibility_validation() {
        use crate::v1;

        // Create a SampleSet with constraints that should make sample 0 infeasible
        // but with provided feasible value claiming it's feasible
        let v1_sample_set = v1::SampleSet {
            decision_variables: vec![],
            objectives: Some(v1::SampledValues {
                entries: vec![v1::sampled_values::SampledValuesEntry {
                    ids: vec![0],
                    value: 10.0,
                }],
            }),
            constraints: vec![v1::SampledConstraint {
                id: 1,
                equality: v1::Equality::EqualToZero as i32,
                evaluated_values: Some(v1::SampledValues {
                    entries: vec![v1::sampled_values::SampledValuesEntry {
                        ids: vec![0],
                        value: 1.0, // This should make constraint infeasible (1.0 != 0.0)
                    }],
                }),
                feasible: [(0, false)].iter().cloned().collect(), // Constraint correctly marked as infeasible
                ..Default::default()
            }],
            feasible: [(0, true)].iter().cloned().collect(), // But overall solution claimed as feasible - inconsistent!
            feasible_relaxed: [(0, true)].iter().cloned().collect(),
            sense: v1::instance::Sense::Minimize as i32,
            ..Default::default()
        };

        let result: Result<SampleSet, ParseError> = v1_sample_set.parse(&());
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error
            .to_string()
            .contains("Inconsistent feasibility for sample 0"));
        assert!(error.to_string().contains("provided=true"));
        assert!(error.to_string().contains("computed=false"));
    }
}
