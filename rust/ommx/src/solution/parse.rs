use super::*;
use crate::{ATol, Parse, ParseError, SolutionError};

impl Parse for crate::v1::Solution {
    type Output = Solution;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.Solution";

        let provided_feasible = self.get_feasible();
        let provided_feasible_relaxed = self.get_feasible_relaxed();

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

        // Parse evaluated constraints
        let mut evaluated_constraints = std::collections::BTreeMap::default();
        for ec in self.evaluated_constraints {
            let parsed_constraint = ec.parse_as(&(), message, "evaluated_constraints")?;
            evaluated_constraints.insert(*parsed_constraint.id(), parsed_constraint);
        }

        let mut decision_variables = std::collections::BTreeMap::default();
        for dv in self.decision_variables {
            // Parse the DecisionVariable to get strongly-typed version
            let parsed_dv = dv.clone().parse_as(&(), message, "decision_variables")?;

            // Get the value from state or substituted_value
            let value = match (state.entries.get(&dv.id), dv.substituted_value.as_ref()) {
                (Some(value), None) | (None, Some(value)) => *value,
                (Some(value), Some(_substituted_value)) => *value, // EvaluatedDecisionVariable::new will check consistency
                (None, None) => {
                    return Err(crate::RawParseError::SolutionError(
                        SolutionError::MissingVariableValue { id: dv.id },
                    )
                    .context(message, "decision_variables"));
                }
            };

            // Use EvaluatedDecisionVariable::new which handles consistency validation
            let evaluated_dv =
                crate::EvaluatedDecisionVariable::new(parsed_dv, value, ATol::default())
                    .map_err(crate::RawParseError::InvalidDecisionVariable)
                    .map_err(|e| ParseError::from(e).context(message, "decision_variables"))?;

            decision_variables.insert(*evaluated_dv.id(), evaluated_dv);
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
            evaluated_constraints,
            decision_variables,
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

impl From<Solution> for crate::v1::Solution {
    fn from(solution: Solution) -> Self {
        let state = solution.state();
        let objective = *solution.objective();
        let evaluated_constraints = solution
            .evaluated_constraints()
            .values()
            .map(|ec| ec.clone().into())
            .collect();
        let decision_variables: Vec<crate::v1::DecisionVariable> = solution
            .decision_variables()
            .values()
            .map(|dv| dv.clone().into())
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
            decision_variables,
            feasible,
            feasible_relaxed,
            optimality,
            relaxation,
            feasible_unrelaxed,
            sense,
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
                id: 1,
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
                id: 1,
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
}
