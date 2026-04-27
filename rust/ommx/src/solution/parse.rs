use super::*;
use crate::{ATol, Parse, ParseError, SolutionError};

impl Parse for crate::v1::Solution {
    type Output = Solution;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.Solution";
        crate::parse::check_format_version(self.format_version, message)?;

        let provided_feasible = match self.feasible_relaxed {
            Some(_) => self.feasible,
            None =>
            {
                #[allow(deprecated)]
                self.feasible_unrelaxed
            }
        };
        let provided_feasible_relaxed = self.feasible_relaxed.unwrap_or(self.feasible);

        let state = self.state.unwrap_or_default();
        let objective = self.objective;

        let v1_sense = crate::v1::instance::Sense::try_from(self.sense)
            .map_err(|_| crate::RawParseError::UnknownEnumValue {
                enum_name: "ommx.v1.Sense",
                value: self.sense,
            })
            .map_err(|e| ParseError::from(e).context(message, "sense"))?;
        let sense = match v1_sense {
            crate::v1::instance::Sense::Unspecified => None,
            crate::v1::instance::Sense::Minimize => Some(crate::Sense::Minimize),
            crate::v1::instance::Sense::Maximize => Some(crate::Sense::Maximize),
        };

        // Parse evaluated constraints and extract removed reasons + metadata
        let mut evaluated_constraints = std::collections::BTreeMap::default();
        let mut removed_reasons = std::collections::BTreeMap::default();
        let mut constraint_metadata =
            crate::ConstraintMetadataStore::<crate::ConstraintID>::default();
        for ec in self.evaluated_constraints {
            let (id, parsed_constraint, metadata, removed_reason): (
                crate::ConstraintID,
                crate::EvaluatedConstraint,
                crate::ConstraintMetadata,
                Option<crate::RemovedReason>,
            ) = ec.parse_as(&(), message, "evaluated_constraints")?;
            if let Some(reason) = removed_reason {
                removed_reasons.insert(id, reason);
            }
            constraint_metadata.insert(id, metadata);
            evaluated_constraints.insert(id, parsed_constraint);
        }
        let mut evaluated_named_functions = std::collections::BTreeMap::default();
        for enf in self.evaluated_named_functions {
            let parsed_named_function = enf.parse_as(&(), message, "evaluated_named_functions")?;
            evaluated_named_functions.insert(parsed_named_function.id(), parsed_named_function);
        }

        let mut decision_variables = std::collections::BTreeMap::default();
        let mut variable_metadata = crate::VariableMetadataStore::default();
        for dv in self.decision_variables {
            let dv_id = dv.id;
            let dv_substituted_value = dv.substituted_value;
            // Parse the DecisionVariable to get strongly-typed version + drained metadata
            let parsed: crate::decision_variable::parse::ParsedDecisionVariable =
                dv.parse_as(&(), message, "decision_variables")?;
            let parsed_dv = parsed.variable;
            let metadata = parsed.metadata;

            // Get the value from state or substituted_value
            let value = match (state.entries.get(&dv_id), dv_substituted_value.as_ref()) {
                (Some(value), None) | (None, Some(value)) => *value,
                (Some(value), Some(_substituted_value)) => *value, // EvaluatedDecisionVariable::new will check consistency
                (None, None) => {
                    return Err(crate::RawParseError::SolutionError(
                        SolutionError::MissingVariableValue { id: dv_id },
                    )
                    .context(message, "decision_variables"));
                }
            };

            // Use EvaluatedDecisionVariable::new which handles consistency validation
            let evaluated_dv =
                crate::EvaluatedDecisionVariable::new(parsed_dv, value, ATol::default())
                    .map_err(crate::RawParseError::InvalidDecisionVariable)
                    .map_err(|e| ParseError::from(e).context(message, "decision_variables"))?;

            let id = *evaluated_dv.id();
            variable_metadata.insert(id, metadata);
            decision_variables.insert(id, evaluated_dv);
        }
        let optimality = self
            .optimality
            .try_into()
            .map_err(|_| crate::RawParseError::UnknownEnumValue {
                enum_name: "ommx.v1.Optimality",
                value: self.optimality,
            })
            .map_err(|e| ParseError::from(e).context(message, "optimality"))?;
        let relaxation = self
            .relaxation
            .try_into()
            .map_err(|_| crate::RawParseError::UnknownEnumValue {
                enum_name: "ommx.v1.Relaxation",
                value: self.relaxation,
            })
            .map_err(|e| ParseError::from(e).context(message, "relaxation"))?;

        let solution = Solution {
            objective,
            evaluated_constraints: crate::constraint_type::EvaluatedCollection::with_metadata(
                evaluated_constraints,
                removed_reasons,
                constraint_metadata,
            ),
            evaluated_indicator_constraints: Default::default(),
            evaluated_one_hot_constraints: Default::default(),
            evaluated_sos1_constraints: Default::default(),
            evaluated_named_functions,
            decision_variables,
            variable_metadata,
            optimality,
            relaxation,
            sense,
        };

        // Validate feasibility consistency
        let computed_feasible = solution.feasible();
        let computed_feasible_relaxed = solution.feasible_relaxed();

        if computed_feasible != provided_feasible {
            return Err(crate::RawParseError::SolutionError(
                SolutionError::InconsistentFeasibility {
                    provided_feasible,
                    computed_feasible,
                },
            )
            .context(message, "feasible"));
        }

        if computed_feasible_relaxed != provided_feasible_relaxed {
            return Err(crate::RawParseError::SolutionError(
                SolutionError::InconsistentFeasibilityRelaxed {
                    provided_feasible_relaxed,
                    computed_feasible_relaxed,
                },
            )
            .context(message, "feasible_relaxed"));
        }

        Ok(solution)
    }
}

/// Lossy: `v1::Solution` only has a `evaluated_constraints` field for
/// regular constraints — it has no fields for indicator / one-hot / sos1
/// evaluated constraints, so any data the in-memory [`Solution`] holds
/// in those collections is dropped on serialization. This is a wire-format
/// limitation that pre-dates the metadata SoA refactor; the matching
/// `Parse` impl above initializes those collections to
/// `Default::default()` for symmetry. Round-trip through `to_bytes` /
/// `from_bytes` preserves variable and regular-constraint metadata.
impl From<Solution> for crate::v1::Solution {
    fn from(solution: Solution) -> Self {
        let state = solution.state();
        let objective = *solution.objective();
        // Drain metadata from the SoA stores and overlay it on per-element
        // proto messages.
        let constraint_metadata_store = solution.evaluated_constraints().metadata().clone();
        let removed_reasons = solution.evaluated_constraints().removed_reasons().clone();
        let evaluated_constraints: Vec<crate::v1::EvaluatedConstraint> = solution
            .evaluated_constraints()
            .iter()
            .map(|(id, ec)| {
                let metadata = constraint_metadata_store.collect_for(*id);
                let mut v1_ec =
                    crate::constraint::evaluated_constraint_to_v1(*id, ec.clone(), metadata);
                if let Some(reason) = removed_reasons.get(id) {
                    v1_ec.removed_reason = Some(reason.reason.clone());
                    v1_ec.removed_reason_parameters = reason
                        .parameters
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();
                }
                v1_ec
            })
            .collect();
        let evaluated_named_functions = solution
            .evaluated_named_functions()
            .values()
            .map(|enf| enf.clone().into())
            .collect();
        let variable_metadata_store = solution.variable_metadata().clone();
        let decision_variables: Vec<crate::v1::DecisionVariable> = solution
            .decision_variables()
            .iter()
            .map(|(id, dv)| {
                let metadata = variable_metadata_store.collect_for(*id);
                crate::decision_variable::evaluated_decision_variable_to_v1(dv.clone(), metadata)
            })
            .collect();
        let feasible = solution.feasible();
        let feasible_relaxed = Some(solution.feasible_relaxed());
        let optimality = solution.optimality.into();
        let relaxation = solution.relaxation.into();
        // For backward compatibility, set feasible_unrelaxed to the same value as feasible
        let feasible_unrelaxed = feasible;
        let sense = match solution.sense {
            None => crate::v1::instance::Sense::Unspecified as i32,
            Some(crate::Sense::Minimize) => crate::v1::instance::Sense::Minimize as i32,
            Some(crate::Sense::Maximize) => crate::v1::instance::Sense::Maximize as i32,
        };

        #[allow(deprecated)]
        crate::v1::Solution {
            state: Some(state),
            objective,
            evaluated_constraints,
            evaluated_named_functions,
            decision_variables,
            feasible,
            feasible_relaxed,
            optimality,
            relaxation,
            feasible_unrelaxed,
            sense,
            format_version: crate::CURRENT_FORMAT_VERSION,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{v1, Parse};

    #[test]
    fn test_solution_parse() {
        let v1_solution = v1::Solution {
            state: Some(v1::State {
                entries: [(1, 2.0), (2, 3.0)].iter().cloned().collect(),
            }),
            objective: 42.5,
            evaluated_constraints: vec![v1::EvaluatedConstraint {
                equality: v1::Equality::EqualToZero as i32,
                evaluated_value: 0.0,
                dual_variable: Some(1.5),
                name: Some("test_constraint".to_string()),
                ..Default::default()
            }],
            decision_variables: vec![v1::DecisionVariable {
                id: 1,
                name: Some("x1".to_string()),
                kind: v1::decision_variable::Kind::Continuous as i32,
                bound: Some(v1::Bound {
                    lower: -100.0,
                    upper: 100.0,
                }),
                ..Default::default()
            }],
            feasible: true,
            feasible_relaxed: Some(true),
            optimality: v1::Optimality::Optimal as i32,
            relaxation: v1::Relaxation::Unspecified as i32,
            sense: v1::instance::Sense::Maximize as i32,
            ..Default::default()
        };

        let parsed: Solution = v1_solution.parse(&()).unwrap();

        assert_eq!(parsed.objective(), &42.5);
        assert!(parsed.feasible());
        assert!(parsed.feasible_relaxed());
        assert_eq!(parsed.optimality, v1::Optimality::Optimal);
        assert_eq!(parsed.relaxation, v1::Relaxation::Unspecified);
        assert_eq!(parsed.evaluated_constraints().len(), 1);
        assert_eq!(parsed.decision_variables().len(), 1);
        assert_eq!(parsed.sense().unwrap(), crate::Sense::Maximize);

        // Test round-trip conversion
        let v1_converted: v1::Solution = parsed.into();
        assert_eq!(v1_converted.objective, 42.5);
        assert!(v1_converted.feasible);
        assert_eq!(v1_converted.feasible_relaxed, Some(true));
        assert_eq!(v1_converted.sense, v1::instance::Sense::Maximize as i32);
    }

    #[test]
    fn test_solution_parser_unspecified_sense() {
        let v1_solution = v1::Solution {
            state: Some(v1::State {
                entries: [(1, 2.0), (2, 3.0)].iter().cloned().collect(),
            }),
            objective: 42.5,
            evaluated_constraints: vec![],
            decision_variables: vec![],
            feasible: true,
            feasible_relaxed: Some(true),
            optimality: v1::Optimality::Optimal as i32,
            relaxation: v1::Relaxation::Unspecified as i32,
            sense: v1::instance::Sense::Unspecified as i32,
            ..Default::default()
        };

        let parsed: Solution = v1_solution.parse(&()).unwrap();
        assert!(parsed.sense().is_none());
    }

    #[test]
    fn test_unknown_sense_enum_value() {
        // Test with an invalid sense value
        let v1_solution = v1::Solution {
            state: None,
            objective: 42.0,
            evaluated_constraints: vec![],
            decision_variables: vec![],
            feasible: true,
            feasible_relaxed: Some(true),
            optimality: v1::Optimality::Optimal as i32,
            relaxation: v1::Relaxation::Unspecified as i32,
            sense: 999, // Unknown enum value
            ..Default::default()
        };

        let result: Result<Solution, ParseError> = v1_solution.parse(&());
        let error = result.unwrap_err();
        insta::assert_snapshot!(error.to_string(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.Solution[sense]
        Unknown or unsupported enum value 999 for ommx.v1.Sense. This may be due to an unspecified value or a newer version of the protocol.
        "###);
    }

    #[test]
    fn test_unknown_enum_value_error() {
        // Test with an invalid optimality value
        let v1_solution = v1::Solution {
            state: None,
            optimality: 99, // Unknown enum value
            relaxation: v1::Relaxation::Unspecified as i32,
            feasible: true,
            ..Default::default()
        };

        let result: Result<Solution, ParseError> = v1_solution.parse(&());
        let error = result.unwrap_err();
        insta::assert_snapshot!(error.to_string(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.Solution[optimality]
        Unknown or unsupported enum value 99 for ommx.v1.Optimality. This may be due to an unspecified value or a newer version of the protocol.
        "###);

        // Test with an invalid relaxation value
        let v1_solution2 = v1::Solution {
            state: None,
            optimality: v1::Optimality::Optimal as i32,
            relaxation: 123, // Unknown enum value
            feasible: true,
            ..Default::default()
        };

        let result2: Result<Solution, ParseError> = v1_solution2.parse(&());
        let error2 = result2.unwrap_err();
        insta::assert_snapshot!(error2.to_string(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.Solution[relaxation]
        Unknown or unsupported enum value 123 for ommx.v1.Relaxation. This may be due to an unspecified value or a newer version of the protocol.
        "###);
    }

    #[test]
    fn test_inconsistent_feasibility_validation() {
        use crate::v1;

        // Create a Solution with constraints that should make it infeasible
        // but with provided feasible value claiming it's feasible
        let v1_solution = v1::Solution {
            state: None, // State can be None when there are no decision variables
            objective: 42.5,
            evaluated_constraints: vec![v1::EvaluatedConstraint {
                equality: v1::Equality::EqualToZero as i32,
                evaluated_value: 1.0, // This should make constraint infeasible (1.0 != 0.0)
                dual_variable: Some(1.5),
                name: Some("test_constraint".to_string()),
                ..Default::default()
            }],
            decision_variables: vec![],
            feasible: true, // But solution claimed as feasible - inconsistent!
            feasible_relaxed: Some(true),
            optimality: v1::Optimality::Optimal as i32,
            relaxation: v1::Relaxation::Unspecified as i32,
            ..Default::default()
        };

        let result: Result<Solution, ParseError> = v1_solution.parse(&());
        let error = result.unwrap_err();
        insta::assert_snapshot!(error.to_string(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.Solution[feasible]
        Inconsistent feasibility for solution: provided=true, computed=false
        "###);
    }

    #[test]
    fn test_inconsistent_variable_value() {
        use crate::v1;

        let v1_solution = v1::Solution {
            state: Some(v1::State {
                entries: [(1, 2.0)].iter().cloned().collect(),
            }),
            objective: 42.5,
            decision_variables: vec![v1::DecisionVariable {
                id: 1,
                substituted_value: Some(3.0), // Different from state value
                kind: v1::decision_variable::Kind::Continuous as i32,
                bound: Some(v1::Bound {
                    lower: 0.0,
                    upper: 10.0,
                }),
                ..Default::default()
            }],
            feasible: true,
            ..Default::default()
        };

        let result: Result<Solution, ParseError> = v1_solution.parse(&());
        let error = result.unwrap_err();
        insta::assert_snapshot!(error.to_string(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.Solution[decision_variables]
        Substituted value for ID=1 cannot be overwritten: previous=3, new=2, atol=ATol(1e-6)
        "###);
    }

    #[test]
    fn test_missing_variable_value() {
        use crate::v1;

        let v1_solution = v1::Solution {
            state: Some(v1::State {
                entries: Default::default(), // Empty state
            }),
            objective: 42.5,
            decision_variables: vec![v1::DecisionVariable {
                id: 1,
                substituted_value: None, // No substituted value either
                kind: v1::decision_variable::Kind::Continuous as i32,
                ..Default::default()
            }],
            feasible: true,
            ..Default::default()
        };

        let result: Result<Solution, ParseError> = v1_solution.parse(&());
        let error = result.unwrap_err();
        insta::assert_snapshot!(error.to_string(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.Solution[decision_variables]
        Missing value for variable 1: not found in state and no substituted_value
        "###);
    }

    // Data produced by a future SDK whose format version exceeds what this SDK supports
    // must be rejected with a clear upgrade-the-SDK error rather than silently misread.
    #[test]
    fn test_solution_parse_rejects_future_format_version() {
        let v1_solution = v1::Solution {
            format_version: 1,
            ..Default::default()
        };
        let result: Result<Solution, ParseError> = v1_solution.parse(&());
        insta::assert_snapshot!(result.unwrap_err().to_string(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.Solution[format_version]
        Unsupported ommx format version: data has format_version=1, but this SDK supports up to 0. Please upgrade the OMMX SDK.
        "###);
    }

    /// Regression: `Solution::to_bytes` / `from_bytes` must preserve the
    /// variable and (regular-constraint) metadata stores. Indicator /
    /// one-hot / sos1 evaluated metadata is dropped because `v1::Solution`
    /// has no fields for those collections — that's a wire-format
    /// limitation older than the SoA refactor and is out of scope here.
    #[test]
    fn test_solution_roundtrip_preserves_metadata() {
        use crate::{
            constraint::EvaluatedData, constraint_type::EvaluatedCollection, ATol, ConstraintID,
            DecisionVariable, Equality, EvaluatedConstraint, EvaluatedDecisionVariable, Sense,
            VariableID,
        };
        use std::collections::BTreeMap;

        let var_id = VariableID::from(1);
        let cid = ConstraintID::from(10);

        let dv = DecisionVariable::binary(var_id);
        let evaluated_dv = EvaluatedDecisionVariable::new(dv, 1.0, ATol::default()).unwrap();
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(var_id, evaluated_dv);

        let mut variable_metadata = crate::VariableMetadataStore::default();
        variable_metadata.set_name(var_id, "x");
        variable_metadata.set_subscripts(var_id, vec![0]);

        let evaluated = EvaluatedConstraint {
            equality: Equality::EqualToZero,
            stage: EvaluatedData {
                evaluated_value: 0.0,
                dual_variable: None,
                feasible: true,
                used_decision_variable_ids: [var_id].into_iter().collect(),
            },
        };
        let mut evaluated_map = BTreeMap::new();
        evaluated_map.insert(cid, evaluated);
        let mut constraint_metadata = crate::ConstraintMetadataStore::<ConstraintID>::default();
        constraint_metadata.set_name(cid, "balance");
        constraint_metadata.set_description(cid, "demand-balance row");
        let evaluated_constraints =
            EvaluatedCollection::with_metadata(evaluated_map, BTreeMap::new(), constraint_metadata);

        // SAFETY: the inputs above satisfy Solution invariants (one DV,
        // one evaluated constraint over that DV, value 1.0 satisfies the
        // equality, no removed reasons).
        let solution = unsafe {
            Solution::builder()
                .objective(1.0)
                .evaluated_constraints_collection(evaluated_constraints)
                .evaluated_named_functions(BTreeMap::new())
                .decision_variables(decision_variables)
                .variable_metadata(variable_metadata)
                .sense(Sense::Minimize)
                .build_unchecked()
                .unwrap()
        };

        let bytes = solution.to_bytes();
        let recovered = Solution::from_bytes(&bytes).unwrap();

        assert_eq!(recovered.variable_metadata().name(var_id), Some("x"));
        assert_eq!(recovered.variable_metadata().subscripts(var_id), &[0]);
        let constraint_meta = recovered.evaluated_constraints().metadata();
        assert_eq!(constraint_meta.name(cid), Some("balance"));
        assert_eq!(constraint_meta.description(cid), Some("demand-balance row"));
    }
}
