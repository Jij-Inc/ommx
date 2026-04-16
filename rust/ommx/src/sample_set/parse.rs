use super::*;
use crate::{Parse, ParseError};
use std::collections::BTreeMap;

impl Parse for crate::v1::SampleSet {
    type Output = SampleSet;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.SampleSet";

        // Parse decision variables into BTreeMap
        let mut decision_variables = BTreeMap::new();
        for v1_sampled_dv in self.decision_variables {
            let sampled_dv = v1_sampled_dv.parse_as(&(), message, "decision_variables")?;
            let dv_id = *sampled_dv.id();
            decision_variables.insert(dv_id, sampled_dv);
        }

        // Parse objectives - required, not optional
        let objectives = self
            .objectives
            .ok_or(
                crate::RawParseError::MissingField {
                    message,
                    field: "objectives",
                }
                .context(message, "objectives"),
            )?
            .parse_as(&(), message, "objectives")?;

        // Parse constraints and extract removed reasons
        let mut constraints = std::collections::BTreeMap::new();
        let mut constraint_removed_reasons = std::collections::BTreeMap::new();
        for v1_constraint in self.constraints {
            let (parsed_constraint, removed_reason): (
                crate::SampledConstraint,
                Option<crate::RemovedReason>,
            ) = v1_constraint.parse_as(&(), message, "constraints")?;
            let id = parsed_constraint.id;
            if let Some(reason) = removed_reason {
                constraint_removed_reasons.insert(id, reason);
            }
            constraints.insert(id, parsed_constraint);
        }

        // Parse named functions into BTreeMap
        let mut named_functions = std::collections::BTreeMap::new();
        for v1_named_function in self.named_functions {
            let parsed_named_function: crate::SampledNamedFunction =
                v1_named_function.parse_as(&(), message, "named_functions")?;
            named_functions.insert(*parsed_named_function.id(), parsed_named_function);
        }

        let sense = self.sense.try_into().map_err(|_| {
            crate::RawParseError::UnknownEnumValue {
                enum_name: "ommx.v1.Sense",
                value: self.sense,
            }
            .context(message, "sense")
        })?;

        // Create SampleSet with validation
        let sample_set = SampleSet::builder()
            .decision_variables(decision_variables)
            .objectives(objectives)
            .constraints_collection(crate::constraint_type::SampledCollection::new(
                constraints,
                constraint_removed_reasons,
            ))
            .named_functions(named_functions)
            .sense(sense)
            .build()
            .map_err(crate::RawParseError::SampleSetError)?;

        // Check the consistency of feasibility maps from the original v1 data
        for (sample_id_u64, provided_feasible) in self.feasible {
            let sample_id = crate::SampleID::from(sample_id_u64);
            if let Ok(computed_feasible) = sample_set.is_sample_feasible(sample_id) {
                if provided_feasible != computed_feasible {
                    return Err(crate::RawParseError::SampleSetError(
                        crate::SampleSetError::InconsistentFeasibility {
                            sample_id: sample_id_u64,
                            provided_feasible,
                            computed_feasible,
                        },
                    )
                    .context(message, "feasible"));
                }
            }
        }

        // Check the consistency of feasible_relaxed maps from the original v1 data
        for (sample_id_u64, provided_feasible_relaxed) in self.feasible_relaxed {
            let sample_id = crate::SampleID::from(sample_id_u64);
            if let Ok(computed_feasible_relaxed) = sample_set.is_sample_feasible_relaxed(sample_id)
            {
                if provided_feasible_relaxed != computed_feasible_relaxed {
                    return Err(crate::RawParseError::SampleSetError(
                        crate::SampleSetError::InconsistentFeasibilityRelaxed {
                            sample_id: sample_id_u64,
                            provided_feasible_relaxed,
                            computed_feasible_relaxed,
                        },
                    )
                    .context(message, "feasible_relaxed"));
                }
            }
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
        let removed_reasons = sample_set.constraints().removed_reasons();
        let constraints: Vec<crate::v1::SampledConstraint> = sample_set
            .constraints()
            .iter()
            .map(|(id, sc)| {
                let mut v1_sc = crate::v1::SampledConstraint::from(sc.clone());
                if let Some(reason) = removed_reasons.get(id) {
                    v1_sc.removed_reason = Some(reason.reason.clone());
                    v1_sc.removed_reason_parameters = reason
                        .parameters
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();
                }
                v1_sc
            })
            .collect();
        let named_functions: Vec<crate::v1::SampledNamedFunction> = sample_set
            .named_functions()
            .values()
            .map(|nf| nf.clone().into())
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
            named_functions,
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
                    bound: Some(v1::Bound {
                        lower: 0.0,
                        upper: 10.0,
                    }),
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
            feasible_relaxed: [(0, true), (1, true), (2, true)].iter().cloned().collect(),
            feasible: [(0, true), (1, true), (2, true)].iter().cloned().collect(),
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
        let error = result.unwrap_err();
        insta::assert_snapshot!(error.to_string(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.SampleSet[sense]
        Unknown or unsupported enum value 999 for ommx.v1.Sense. This may be due to an unspecified value or a newer version of the protocol.
        "###);
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
        let error = result.unwrap_err();
        insta::assert_snapshot!(error.to_string(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.SampleSet[feasible]
        Inconsistent feasibility for sample 0: provided=true, computed=false
        "###);
    }
}
