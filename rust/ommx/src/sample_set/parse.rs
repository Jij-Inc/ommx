use super::*;
use crate::{Parse, ParseError};
use std::collections::BTreeMap;

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
            decision_variables.insert(dv_id, parsed.variable);
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
            let id = *parsed.sampled_named_function.id();
            named_functions.insert(id, parsed.sampled_named_function);
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
        // Drain labels/context and overlay onto per-element messages.
        let variable_labels = sample_set.variable_labels().clone();
        let decision_variables: Vec<crate::v1::SampledDecisionVariable> = sample_set
            .decision_variables()
            .iter()
            .map(|(id, dv)| {
                let label = variable_labels.collect_for(*id);
                crate::decision_variable::sampled_decision_variable_to_v1(*id, dv.clone(), label)
            })
            .collect();
        let objectives = Some(sample_set.objectives().clone().into());
        let constraint_context = sample_set.constraints().context().clone();
        let removed_reasons = sample_set.constraints().removed_reasons().clone();
        let constraints: Vec<crate::v1::SampledConstraint> = sample_set
            .constraints()
            .iter()
            .map(|(id, sc)| {
                let context = constraint_context.collect_for(*id);
                let mut v1_sc =
                    crate::constraint::sampled_constraint_to_v1(*id, sc.clone(), context);
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
        let named_function_labels_store = sample_set.named_function_labels().clone();
        let named_functions: Vec<crate::v1::SampledNamedFunction> = sample_set
            .named_functions()
            .iter()
            .map(|(id, nf)| {
                let label = named_function_labels_store.collect_for(*id);
                crate::named_function::parse::sampled_named_function_to_v1(nf.clone(), label)
            })
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
            format_version: crate::CURRENT_FORMAT_VERSION,
            metadata: sample_set.metadata,
            annotations: crate::protobuf_extension_annotations(sample_set.annotations),
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
}
