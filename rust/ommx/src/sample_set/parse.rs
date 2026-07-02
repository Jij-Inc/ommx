use super::*;
use crate::{v2, Parse, ParseError, RawParseError};
use std::collections::{BTreeMap, BTreeSet};

fn sampled_collection_has_payload<T: crate::ConstraintType>(
    collection: &crate::constraint_type::SampledCollection<T>,
) -> bool {
    !collection.is_empty()
}

fn validate_sampled_indicator_structural_ids(
    constraints: &crate::constraint_type::SampledCollection<crate::IndicatorConstraint>,
    decision_variables: &crate::SampledDecisionVariableTable,
    message: &'static str,
) -> Result<(), ParseError> {
    for (constraint_id, constraint) in constraints.inner() {
        let id = constraint.indicator_variable;
        let Some(variable) = decision_variables.get(&id) else {
            return Err(RawParseError::InvalidInstance(format!(
                "Indicator variable {id:?} in constraint {constraint_id:?} is not defined in decision_variables",
            ))
            .context(message, "sampled_indicator_constraints"));
        };
        if *variable.kind() != crate::decision_variable::Kind::Binary {
            return Err(RawParseError::InvalidInstance(format!(
                "Indicator variable {id:?} in constraint {constraint_id:?} must be binary",
            ))
            .context(message, "sampled_indicator_constraints"));
        }
    }
    Ok(())
}

fn validate_sampled_one_hot_structural_ids(
    constraints: &crate::constraint_type::SampledCollection<crate::OneHotConstraint>,
    decision_variables: &crate::SampledDecisionVariableTable,
    message: &'static str,
) -> Result<(), ParseError> {
    for (constraint_id, constraint) in constraints.inner() {
        for id in &constraint.variables {
            let Some(variable) = decision_variables.get(id) else {
                return Err(RawParseError::InvalidInstance(format!(
                    "One-hot variable {id:?} in constraint {constraint_id:?} is not defined in decision_variables",
                ))
                .context(message, "sampled_one_hot_constraints"));
            };
            if *variable.kind() != crate::decision_variable::Kind::Binary {
                return Err(RawParseError::InvalidInstance(format!(
                    "One-hot variable {id:?} in constraint {constraint_id:?} must be binary",
                ))
                .context(message, "sampled_one_hot_constraints"));
            }
        }
    }
    Ok(())
}

fn validate_sampled_sos1_structural_ids(
    constraints: &crate::constraint_type::SampledCollection<crate::Sos1Constraint>,
    decision_variables: &crate::SampledDecisionVariableTable,
    message: &'static str,
) -> Result<(), ParseError> {
    for (constraint_id, constraint) in constraints.inner() {
        for id in &constraint.variables {
            if !decision_variables.contains_key(id) {
                return Err(RawParseError::InvalidInstance(format!(
                    "SOS1 variable {id:?} in constraint {constraint_id:?} is not defined in decision_variables",
                ))
                .context(message, "sampled_sos1_constraints"));
            }
        }
    }
    Ok(())
}

fn first_feasibility_mismatch(
    provided: &BTreeMap<SampleID, bool>,
    computed: &BTreeMap<SampleID, bool>,
) -> Option<(SampleID, bool, bool)> {
    computed.iter().find_map(|(id, computed)| {
        let provided = provided
            .get(id)
            .copied()
            .expect("feasibility maps must be validated to have identical sample IDs");
        let computed = *computed;
        (provided != computed).then_some((*id, provided, computed))
    })
}

fn validate_sample_bool_map_ids(
    map: &BTreeMap<SampleID, bool>,
    expected: &SampleIDSet,
    message: &'static str,
    field: &'static str,
) -> Result<(), ParseError> {
    let found = map.keys().copied().collect::<SampleIDSet>();
    if &found != expected {
        return Err(
            RawParseError::SampleSetError(crate::SampleSetError::InconsistentSampleIDs {
                expected: expected.clone(),
                found,
            })
            .context(message, field),
        );
    }
    Ok(())
}

impl Parse for crate::v1::SampleSet {
    type Output = SampleSet;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.SampleSet";
        crate::parse::check_format_version(self.format_version, message)?;
        crate::parse::validate_extension_annotations(&self.annotations, message)?;

        // Parse decision variables into BTreeMap and drain labels into the SoA store
        let mut decision_variables = BTreeMap::new();
        let mut variable_labels = crate::VariableLabelStore::default();
        for v1_sampled_dv in self.decision_variables {
            let parsed: crate::decision_variable::parse::ParsedSampledDecisionVariable =
                v1_sampled_dv.parse_as(&(), message, "decision_variables")?;
            let dv_id = parsed.id;
            variable_labels.insert(dv_id, parsed.label);
            if decision_variables.insert(dv_id, parsed.variable).is_some() {
                return Err(crate::RawParseError::SampleSetError(
                    crate::SampleSetError::DuplicatedVariableID { id: dv_id },
                )
                .context(message, "decision_variables"));
            }
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

        // Parse constraints and extract removed reasons + context
        let mut constraints = std::collections::BTreeMap::new();
        let mut constraint_removed_reasons = std::collections::BTreeMap::new();
        let mut constraint_context =
            crate::ConstraintContextStore::<crate::ConstraintID>::default();
        for v1_constraint in self.constraints {
            let (id, parsed_constraint, context, removed_reason): (
                crate::ConstraintID,
                crate::SampledConstraint,
                crate::ConstraintContext,
                Option<crate::RemovedReason>,
            ) = v1_constraint.parse_as(&(), message, "constraints")?;
            if let Some(reason) = removed_reason {
                constraint_removed_reasons.insert(id, reason);
            }
            constraint_context.insert(id, context);
            constraints.insert(id, parsed_constraint);
        }

        // Parse named functions into BTreeMap, draining labels into the SoA store
        let mut named_functions = std::collections::BTreeMap::new();
        let mut named_function_labels = crate::named_function::NamedFunctionLabelStore::default();
        for v1_named_function in self.named_functions {
            let parsed: crate::named_function::parse::ParsedSampledNamedFunction =
                v1_named_function.parse_as(&(), message, "named_functions")?;
            let id = parsed.id;
            if named_functions
                .insert(id, parsed.sampled_named_function)
                .is_some()
            {
                return Err(crate::RawParseError::SampleSetError(
                    crate::SampleSetError::DuplicatedNamedFunctionID { id },
                )
                .context(message, "named_functions"));
            }
            named_function_labels.insert(id, parsed.label);
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
            .variable_labels(variable_labels)
            .objectives(objectives)
            .constraints_collection(
                crate::constraint_type::SampledCollection::with_context(
                    constraints,
                    constraint_removed_reasons,
                    constraint_context,
                )
                .map_err(|e| {
                    crate::RawParseError::InvalidInstance(e.to_string())
                        .context(message, "constraints")
                })?,
            )
            .named_functions(named_functions)
            .named_function_labels(named_function_labels)
            .sense(sense)
            .build()
            .map_err(crate::RawParseError::SampleSetError)?;
        let mut sample_set = sample_set;
        sample_set.metadata = self.metadata;
        sample_set.annotations = self.annotations;

        // Check the consistency of feasibility maps from the original v1 data
        for (sample_id_u64, provided_feasible) in self.feasible {
            let sample_id = crate::SampleID::from(sample_id_u64);
            if let Some(computed_feasible) = sample_set.is_sample_feasible(sample_id) {
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
            if let Some(computed_feasible_relaxed) =
                sample_set.is_sample_feasible_relaxed(sample_id)
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

impl Parse for v2::SampleSet {
    type Output = SampleSet;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v2.SampleSet";
        let required_features =
            crate::v2_io::parse_required_features(self.required_features, message)?;
        let feasibility_atol =
            crate::v2_io::parse_feasibility_atol(self.feasibility_atol, message)?;
        let annotations =
            crate::v2_io::extension_annotations_from_v2_map(self.annotations, message)?;
        let decision_variables = self
            .decision_variables
            .ok_or(RawParseError::MissingField {
                message,
                field: "decision_variables",
            })?
            .parse_as(&(), message, "decision_variables")?;
        let objectives = self
            .objectives
            .ok_or(RawParseError::MissingField {
                message,
                field: "objectives",
            })?
            .parse_as(&(), message, "objectives")?;
        crate::v2_io::validate_sampled_f64_values(&objectives, message, "objectives")?;
        let constraints = self
            .sampled_regular_constraints
            .map(|value| value.parse_as(&feasibility_atol, message, "sampled_regular_constraints"))
            .transpose()?
            .unwrap_or_default();
        let indicator_constraints = self
            .sampled_indicator_constraints
            .map(|value| {
                value.parse_as(&feasibility_atol, message, "sampled_indicator_constraints")
            })
            .transpose()?
            .unwrap_or_default();
        let one_hot_constraints = self
            .sampled_one_hot_constraints
            .map(|value| value.parse_as(&feasibility_atol, message, "sampled_one_hot_constraints"))
            .transpose()?
            .unwrap_or_default();
        let sos1_constraints = self
            .sampled_sos1_constraints
            .map(|value| value.parse_as(&feasibility_atol, message, "sampled_sos1_constraints"))
            .transpose()?
            .unwrap_or_default();

        crate::v2_io::validate_feature_payload(
            &required_features,
            v2::Feature::ConstraintIndicator,
            sampled_collection_has_payload(&indicator_constraints),
            message,
            "sampled_indicator_constraints",
        )?;
        crate::v2_io::validate_feature_payload(
            &required_features,
            v2::Feature::ConstraintOneHot,
            sampled_collection_has_payload(&one_hot_constraints),
            message,
            "sampled_one_hot_constraints",
        )?;
        crate::v2_io::validate_feature_payload(
            &required_features,
            v2::Feature::ConstraintSos1,
            sampled_collection_has_payload(&sos1_constraints),
            message,
            "sampled_sos1_constraints",
        )?;

        let named_functions = self
            .sampled_named_functions
            .map(|value| value.parse_as(&(), message, "sampled_named_functions"))
            .transpose()?
            .unwrap_or_default();
        let sense = self.sense.try_into().map_err(|_| {
            RawParseError::UnknownEnumValue {
                enum_name: "ommx.v1.Sense",
                value: self.sense,
            }
            .context(message, "sense")
        })?;

        let objective_sample_ids = objectives.ids();
        for sampled_dv in decision_variables.values() {
            if !sampled_dv.samples().has_same_ids(&objective_sample_ids) {
                return Err(RawParseError::SampleSetError(
                    crate::SampleSetError::InconsistentSampleIDs {
                        expected: objective_sample_ids.clone(),
                        found: sampled_dv.samples().ids(),
                    },
                )
                .context(message, "decision_variables"));
            }
        }
        constraints
            .validate_sample_ids(&objective_sample_ids)
            .map_err(|found| {
                RawParseError::SampleSetError(crate::SampleSetError::InconsistentSampleIDs {
                    expected: objective_sample_ids.clone(),
                    found,
                })
                .context(message, "sampled_regular_constraints")
            })?;
        indicator_constraints
            .validate_sample_ids(&objective_sample_ids)
            .map_err(|found| {
                RawParseError::SampleSetError(crate::SampleSetError::InconsistentSampleIDs {
                    expected: objective_sample_ids.clone(),
                    found,
                })
                .context(message, "sampled_indicator_constraints")
            })?;
        one_hot_constraints
            .validate_sample_ids(&objective_sample_ids)
            .map_err(|found| {
                RawParseError::SampleSetError(crate::SampleSetError::InconsistentSampleIDs {
                    expected: objective_sample_ids.clone(),
                    found,
                })
                .context(message, "sampled_one_hot_constraints")
            })?;
        sos1_constraints
            .validate_sample_ids(&objective_sample_ids)
            .map_err(|found| {
                RawParseError::SampleSetError(crate::SampleSetError::InconsistentSampleIDs {
                    expected: objective_sample_ids.clone(),
                    found,
                })
                .context(message, "sampled_sos1_constraints")
            })?;

        let decision_variable_ids = decision_variables.keys().copied().collect::<BTreeSet<_>>();
        validate_sampled_constraint_used_ids("regular", &constraints, &decision_variable_ids)
            .map_err(|e| {
                RawParseError::SampleSetError(e).context(message, "sampled_regular_constraints")
            })?;
        validate_sampled_constraint_used_ids(
            "indicator",
            &indicator_constraints,
            &decision_variable_ids,
        )
        .map_err(|e| {
            RawParseError::SampleSetError(e).context(message, "sampled_indicator_constraints")
        })?;
        validate_sampled_constraint_used_ids(
            "one-hot",
            &one_hot_constraints,
            &decision_variable_ids,
        )
        .map_err(|e| {
            RawParseError::SampleSetError(e).context(message, "sampled_one_hot_constraints")
        })?;
        validate_sampled_constraint_used_ids("SOS1", &sos1_constraints, &decision_variable_ids)
            .map_err(|e| {
                RawParseError::SampleSetError(e).context(message, "sampled_sos1_constraints")
            })?;
        validate_sampled_indicator_structural_ids(
            &indicator_constraints,
            &decision_variables,
            message,
        )?;
        validate_sampled_one_hot_structural_ids(
            &one_hot_constraints,
            &decision_variables,
            message,
        )?;
        validate_sampled_sos1_structural_ids(&sos1_constraints, &decision_variables, message)?;

        for (named_function_id, sampled_named_function) in named_functions.iter() {
            if !sampled_named_function
                .evaluated_values()
                .has_same_ids(&objective_sample_ids)
            {
                return Err(RawParseError::SampleSetError(
                    crate::SampleSetError::InconsistentSampleIDs {
                        expected: objective_sample_ids.clone(),
                        found: sampled_named_function.evaluated_values().ids(),
                    },
                )
                .context(message, "sampled_named_functions"));
            }
            for var_id in sampled_named_function.used_decision_variable_ids() {
                if !decision_variables.contains_key(var_id) {
                    return Err(RawParseError::SampleSetError(
                        crate::SampleSetError::UndefinedVariableInNamedFunction {
                            id: *var_id,
                            named_function_id: *named_function_id,
                        },
                    )
                    .context(message, "sampled_named_functions"));
                }
            }
        }

        let (computed_feasible, computed_feasible_relaxed) = SampleSetBuilder::compute_feasibility(
            &constraints,
            &indicator_constraints,
            &one_hot_constraints,
            &sos1_constraints,
            &objective_sample_ids,
        );
        let feasible = crate::v2_io::sample_bool_map_from_v2(self.feasible);
        validate_sample_bool_map_ids(&feasible, &objective_sample_ids, message, "feasible")?;
        if let Some((sample_id, provided_feasible, computed_feasible)) =
            first_feasibility_mismatch(&feasible, &computed_feasible)
        {
            return Err(RawParseError::SampleSetError(
                crate::SampleSetError::InconsistentFeasibility {
                    sample_id: sample_id.into_inner(),
                    provided_feasible,
                    computed_feasible,
                },
            )
            .context(message, "feasible"));
        }
        let feasible_relaxed = crate::v2_io::sample_bool_map_from_v2(self.feasible_relaxed);
        validate_sample_bool_map_ids(
            &feasible_relaxed,
            &objective_sample_ids,
            message,
            "feasible_relaxed",
        )?;
        if let Some((sample_id, provided_feasible_relaxed, computed_feasible_relaxed)) =
            first_feasibility_mismatch(&feasible_relaxed, &computed_feasible_relaxed)
        {
            return Err(RawParseError::SampleSetError(
                crate::SampleSetError::InconsistentFeasibilityRelaxed {
                    sample_id: sample_id.into_inner(),
                    provided_feasible_relaxed,
                    computed_feasible_relaxed,
                },
            )
            .context(message, "feasible_relaxed"));
        }

        Ok(SampleSet {
            decision_variables,
            objectives,
            constraints,
            indicator_constraints,
            one_hot_constraints,
            sos1_constraints,
            named_functions,
            sense,
            feasible,
            feasible_relaxed,
            feasibility_atol,
            metadata: self.metadata,
            annotations,
        })
    }
}

impl TryFrom<v2::SampleSet> for SampleSet {
    type Error = ParseError;

    fn try_from(value: v2::SampleSet) -> Result<Self, Self::Error> {
        value.parse(&())
    }
}

/// Lossy: `v1::SampleSet` only has a `constraints` field for regular
/// sampled constraints — it has no fields for indicator / one-hot / sos1
/// sampled constraints, so any data the in-memory [`SampleSet`] holds in
/// those collections is dropped on serialization. This is a wire-format
/// limitation that pre-dates the label/context SoA refactor; the matching
/// `Parse` impl above initializes those collections to
/// `Default::default()` for symmetry. Round-trip through `to_bytes` /
/// `from_bytes` preserves variable labels and regular-constraint context.
impl From<SampleSet> for crate::v1::SampleSet {
    fn from(sample_set: SampleSet) -> Self {
        let SampleSet {
            decision_variables,
            objectives,
            constraints,
            indicator_constraints: _,
            one_hot_constraints: _,
            sos1_constraints: _,
            named_functions,
            sense,
            feasible,
            feasible_relaxed,
            feasibility_atol: _,
            metadata,
            annotations,
        } = sample_set;
        let decision_variables: Vec<crate::v1::SampledDecisionVariable> =
            (&decision_variables).into();
        let objectives = Some(objectives.into());
        let constraints: Vec<crate::v1::SampledConstraint> = constraints.into();
        let named_functions: Vec<crate::v1::SampledNamedFunction> = named_functions.into();
        let sense = sense.into();
        let feasible = feasible
            .into_iter()
            .map(|(sample_id, value)| (sample_id.into_inner(), value))
            .collect();
        let feasible_relaxed = feasible_relaxed
            .into_iter()
            .map(|(sample_id, value)| (sample_id.into_inner(), value))
            .collect();

        crate::v1::SampleSet {
            decision_variables,
            objectives,
            constraints,
            named_functions,
            feasible_relaxed,
            feasible,
            sense,
            format_version: crate::CURRENT_FORMAT_VERSION,
            metadata,
            annotations: crate::protobuf_extension_annotations(annotations),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{v1, Parse};

    #[test]
    fn test_sample_set_parse_rejects_reserved_annotation_key() {
        let v1_sample_set = v1::SampleSet {
            annotations: std::collections::HashMap::from([(
                format!("{}.solver", crate::annotation_keys::SAMPLE_SET_NAMESPACE),
                "bad".to_string(),
            )]),
            ..Default::default()
        };
        let result: Result<SampleSet, ParseError> = v1_sample_set.parse(&());
        insta::assert_snapshot!(result.unwrap_err().to_string(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.SampleSet[annotations]
        Annotation key `org.ommx.v1.sample-set.solver` is reserved for OMMX metadata and cannot be stored in extension annotations.
        "###);
    }

    #[test]
    fn test_sample_set_to_bytes_filters_reserved_annotation_key() {
        let mut sample_set: SampleSet = v1::SampleSet {
            objectives: Some(v1::SampledValues {
                entries: vec![v1::sampled_values::SampledValuesEntry {
                    ids: vec![0],
                    value: 1.0,
                }],
            }),
            sense: v1::instance::Sense::Minimize as i32,
            ..Default::default()
        }
        .parse(&())
        .unwrap();
        let reserved_key = format!("{}.solver", crate::annotation_keys::SAMPLE_SET_NAMESPACE);
        sample_set.annotations = std::collections::HashMap::from([
            (reserved_key.clone(), "invalid extension solver".to_string()),
            ("org.example.owner".to_string(), "domain".to_string()),
        ]);

        let restored = SampleSet::from_bytes(&sample_set.to_bytes()).unwrap();

        assert!(!restored.annotations.contains_key(&reserved_key));
        assert_eq!(
            restored.annotations.get("org.example.owner"),
            Some(&"domain".to_string())
        );
    }

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
        assert!(parsed.is_sample_feasible(unknown_sample_id).is_none());
        assert!(parsed
            .is_sample_feasible_relaxed(unknown_sample_id)
            .is_none());

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

    // Data produced by a future SDK whose format version exceeds what this SDK supports
    // must be rejected with a clear upgrade-the-SDK error rather than silently misread.
    #[test]
    fn test_sample_set_parse_rejects_future_format_version() {
        let v1_sample_set = v1::SampleSet {
            format_version: 1,
            ..Default::default()
        };
        let result: Result<SampleSet, ParseError> = v1_sample_set.parse(&());
        insta::assert_snapshot!(result.unwrap_err().to_string(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.SampleSet[format_version]
        Unsupported ommx format version: data has format_version=1, but this SDK supports up to 0. Please upgrade the OMMX SDK.
        "###);
    }

    #[test]
    fn test_sample_set_parse_fails_with_duplicated_variable_id() {
        let sample_id = crate::SampleID::from(0);
        let v1_sampled_dv = crate::v1::SampledDecisionVariable {
            decision_variable: Some(crate::v1::DecisionVariable {
                id: 1,
                kind: crate::v1::decision_variable::Kind::Continuous as i32,
                bound: Some(crate::v1::Bound {
                    lower: 0.0,
                    upper: 10.0,
                }),
                ..Default::default()
            }),
            samples: Some(crate::v1::SampledValues {
                entries: vec![crate::v1::sampled_values::SampledValuesEntry {
                    ids: vec![sample_id.into_inner()],
                    value: 2.0,
                }],
            }),
        };
        let v1_sample_set = crate::v1::SampleSet {
            decision_variables: vec![v1_sampled_dv.clone(), v1_sampled_dv],
            objectives: Some(crate::v1::SampledValues {
                entries: vec![crate::v1::sampled_values::SampledValuesEntry {
                    ids: vec![sample_id.into_inner()],
                    value: 0.0,
                }],
            }),
            sense: crate::v1::instance::Sense::Minimize as i32,
            ..Default::default()
        };

        let result: Result<SampleSet, ParseError> = v1_sample_set.parse(&());
        let error = result.unwrap_err();
        assert!(matches!(
            error.error,
            crate::RawParseError::SampleSetError(crate::SampleSetError::DuplicatedVariableID { id })
                if id == crate::VariableID::from(1)
        ));
        insta::assert_snapshot!(error.to_string(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.SampleSet[decision_variables]
        Duplicated variable ID is found in definition: VariableID(1)
        "###);
    }

    #[test]
    fn test_sample_set_parse_fails_with_duplicated_named_function_id() {
        let sample_id = crate::SampleID::from(0);
        let sampled_values = crate::v1::SampledValues {
            entries: vec![crate::v1::sampled_values::SampledValuesEntry {
                ids: vec![sample_id.into_inner()],
                value: 1.0,
            }],
        };
        let v1_sample_set = crate::v1::SampleSet {
            objectives: Some(crate::v1::SampledValues {
                entries: vec![crate::v1::sampled_values::SampledValuesEntry {
                    ids: vec![sample_id.into_inner()],
                    value: 0.0,
                }],
            }),
            named_functions: vec![
                crate::v1::SampledNamedFunction {
                    id: 7,
                    evaluated_values: Some(sampled_values.clone()),
                    ..Default::default()
                },
                crate::v1::SampledNamedFunction {
                    id: 7,
                    evaluated_values: Some(sampled_values),
                    ..Default::default()
                },
            ],
            sense: crate::v1::instance::Sense::Minimize as i32,
            ..Default::default()
        };

        let result: Result<SampleSet, ParseError> = v1_sample_set.parse(&());
        let error = result.unwrap_err();
        assert!(matches!(
            error.error,
            crate::RawParseError::SampleSetError(
                crate::SampleSetError::DuplicatedNamedFunctionID { id }
            ) if id == crate::NamedFunctionID::from(7)
        ));
        insta::assert_snapshot!(error.to_string(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.SampleSet[named_functions]
        Duplicated named function ID is found in definition: NamedFunctionID(7)
        "###);
    }

    /// Regression: `SampleSet::to_bytes` / `from_bytes` must preserve the
    /// variable-label and regular-constraint-context stores. Indicator /
    /// one-hot / sos1 sampled context is dropped because `v1::SampleSet`
    /// has no fields for those collections — that's a wire-format
    /// limitation older than the SoA refactor and is out of scope here.
    #[test]
    fn test_sample_set_roundtrip_preserves_labels_and_context() {
        use crate::constraint::SampledData;
        use crate::{
            ConstraintID, DecisionVariable, Equality, NamedFunctionID, SampleID,
            SampledDecisionVariable, Sense, VariableID,
        };
        use std::collections::BTreeMap;

        let var_id = VariableID::from(1);
        let cid = ConstraintID::from(10);
        let nf_id = NamedFunctionID::from(0);
        let sample_id = SampleID::from(0);

        let dv = DecisionVariable::binary();
        let mut x_samples = crate::Sampled::default();
        x_samples.append([sample_id], 1.0).unwrap();
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(
            var_id,
            SampledDecisionVariable::new(var_id, dv, x_samples).unwrap(),
        );

        let mut variable_labels = crate::VariableLabelStore::default();
        variable_labels.set_name(var_id, "x");
        variable_labels.set_subscripts(var_id, vec![0]);

        let mut evaluated_values = crate::Sampled::default();
        evaluated_values.append([sample_id], 0.0).unwrap();
        let mut feasible = BTreeMap::new();
        feasible.insert(sample_id, true);
        let sampled_constraint = crate::Constraint {
            equality: Equality::EqualToZero,
            stage: SampledData {
                evaluated_values,
                dual_variables: None,
                feasible,
                used_decision_variable_ids: [var_id].into_iter().collect(),
            },
        };
        let mut constraints_map = BTreeMap::new();
        constraints_map.insert(cid, sampled_constraint);
        let mut constraint_context = crate::ConstraintContextStore::<ConstraintID>::default();
        constraint_context.set_name(cid, "balance");
        constraint_context.set_description(cid, "demand-balance row");
        let constraints = crate::constraint_type::SampledCollection::with_context(
            constraints_map,
            BTreeMap::new(),
            constraint_context,
        )
        .unwrap();

        // Add a sampled named function with a non-empty label so the
        // round-trip exercises the named_function_labels SoA store too.
        // `SampledNamedFunction` has module-private fields; construct via
        // the v1 parse helper (same path Instance::evaluate_samples uses).
        let sampled_nf = {
            use crate::parse::Parse as _;
            let v1_snf = crate::v1::SampledNamedFunction {
                id: nf_id.into_inner(),
                evaluated_values: Some(crate::v1::SampledValues {
                    entries: vec![crate::v1::sampled_values::SampledValuesEntry {
                        ids: vec![sample_id.into_inner()],
                        value: 1.0,
                    }],
                }),
                used_decision_variable_ids: vec![var_id.into_inner()],
                ..Default::default()
            };
            let parsed: crate::named_function::parse::ParsedSampledNamedFunction =
                v1_snf.parse(&()).unwrap();
            parsed.sampled_named_function
        };
        let mut named_functions = BTreeMap::new();
        named_functions.insert(nf_id, sampled_nf);
        let mut named_function_labels = crate::named_function::NamedFunctionLabelStore::default();
        named_function_labels.set_name(nf_id, "offset_x");
        named_function_labels.set_subscripts(nf_id, vec![0]);
        named_function_labels.set_description(nf_id, "x plus a constant");

        let mut objectives = crate::Sampled::default();
        objectives.append([sample_id], 1.0).unwrap();

        let sample_set = SampleSet::builder()
            .decision_variables(decision_variables)
            .variable_labels(variable_labels)
            .objectives(objectives)
            .constraints_collection(constraints)
            .named_functions(named_functions)
            .named_function_labels(named_function_labels)
            .sense(Sense::Minimize)
            .build()
            .unwrap();

        let bytes = sample_set.to_bytes();
        let recovered = SampleSet::from_bytes(&bytes).unwrap();

        assert_eq!(recovered.variable_labels().name(var_id), Some("x"));
        assert_eq!(recovered.variable_labels().subscripts(var_id), &[0]);
        let constraint_meta = recovered.constraints().context();
        assert_eq!(constraint_meta.name(cid), Some("balance"));
        assert_eq!(constraint_meta.description(cid), Some("demand-balance row"));
        let nf_meta = recovered.named_function_labels();
        assert_eq!(nf_meta.name(nf_id), Some("offset_x"));
        assert_eq!(nf_meta.subscripts(nf_id), &[0]);
        assert_eq!(nf_meta.description(nf_id), Some("x plus a constant"));
    }

    #[test]
    fn test_v2_sample_set_parse_rejects_inconsistent_regular_feasibility() {
        use crate::{
            constraint::SampledData, Constraint, ConstraintID, Equality, SampleID, Sampled, Sense,
        };
        use std::collections::BTreeMap;

        let sample_id = SampleID::from(0);
        let mut objectives = Sampled::default();
        objectives.append([sample_id], 0.0).unwrap();

        let mut evaluated_values = Sampled::default();
        evaluated_values.append([sample_id], 0.0).unwrap();
        let constraint = Constraint {
            equality: Equality::EqualToZero,
            stage: SampledData {
                evaluated_values,
                feasible: BTreeMap::from([(sample_id, true)]),
                used_decision_variable_ids: Default::default(),
                dual_variables: None,
            },
        };
        let sample_set = SampleSet::builder()
            .decision_variables(BTreeMap::new())
            .objectives(objectives)
            .constraints(BTreeMap::from([(ConstraintID::from(1), constraint)]))
            .sense(Sense::Minimize)
            .build()
            .unwrap();

        let mut proto = crate::v2::SampleSet::from(sample_set);
        let row = proto
            .sampled_regular_constraints
            .as_mut()
            .unwrap()
            .entries
            .get_mut(&1)
            .unwrap();
        row.evaluated_values
            .as_mut()
            .unwrap()
            .entries
            .first_mut()
            .unwrap()
            .value = 1.0;
        row.feasible.insert(sample_id.into_inner(), true);

        let err = SampleSet::try_from(proto).unwrap_err();
        assert!(
            err.to_string()
                .contains("Inconsistent constraint feasibility"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_v2_sample_set_parse_rejects_non_finite_objective() {
        use crate::{SampleID, Sampled, Sense};
        use std::collections::BTreeMap;

        let sample_id = SampleID::from(0);
        let mut objectives = Sampled::default();
        objectives.append([sample_id], 0.0).unwrap();
        let sample_set = SampleSet::builder()
            .decision_variables(BTreeMap::new())
            .objectives(objectives)
            .constraints(BTreeMap::new())
            .sense(Sense::Minimize)
            .build()
            .unwrap();

        let mut proto = crate::v2::SampleSet::from(sample_set);
        proto
            .objectives
            .as_mut()
            .unwrap()
            .entries
            .first_mut()
            .unwrap()
            .value = f64::INFINITY;

        let err = SampleSet::try_from(proto).unwrap_err();
        assert!(
            err.to_string().contains("objectives must be finite"),
            "unexpected error: {err}"
        );
    }
}
