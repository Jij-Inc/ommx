use super::*;
use crate::{Parse, ParseError, RawParseError};

impl Parse for crate::v1::Solution {
    type Output = Solution;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let state = self.state.unwrap_or_default();
        let objective = self.objective;

        // Parse evaluated constraints
        let evaluated_constraints: Result<Vec<EvaluatedConstraint>, ParseError> = self
            .evaluated_constraints
            .into_iter()
            .map(|ec| ec.parse(&()))
            .collect();
        let evaluated_constraints = evaluated_constraints?;

        let decision_variables: Result<Vec<_>, ParseError> = self
            .decision_variables
            .into_iter()
            .map(|dv| {
                let parsed: crate::DecisionVariable = dv.parse(&())?;
                // For parsing, we need to extract the value from substituted_value
                let value = parsed.substituted_value().unwrap_or(0.0);
                Ok(crate::EvaluatedDecisionVariable::new_internal(
                    parsed.id(),
                    parsed.kind(),
                    parsed.bound(),
                    value,
                    crate::DecisionVariableMetadata {
                        name: parsed.name.clone(),
                        subscripts: parsed.subscripts.clone(),
                        parameters: parsed.parameters.clone(),
                        description: parsed.description.clone(),
                    },
                ))
            })
            .collect();
        let decision_variables = decision_variables?;
        let (feasible, feasible_relaxed) = match self.feasible_relaxed {
            Some(feasible_relaxed) => {
                // New format since OMMX Python SDK 1.7.0
                // https://github.com/Jij-Inc/ommx/pull/280
                (self.feasible, feasible_relaxed)
            }
            None => {
                // Before OMMX Python SDK 1.7.0, the `feasible` field means current `feasible_relaxed`,
                // and the deprecated `feasible_unrelaxed` is the same as `feasible`.
                #[allow(deprecated)]
                (self.feasible_unrelaxed, self.feasible)
            }
        };

        let optimality =
            self.optimality
                .try_into()
                .map_err(|_| RawParseError::UnknownEnumValue {
                    enum_name: "ommx.v1.Optimality",
                    value: self.optimality,
                })?;
        let relaxation =
            self.relaxation
                .try_into()
                .map_err(|_| RawParseError::UnknownEnumValue {
                    enum_name: "ommx.v1.Relaxation",
                    value: self.relaxation,
                })?;

        Ok(Solution {
            state,
            objective,
            evaluated_constraints,
            decision_variables,
            feasible,
            feasible_relaxed,
            optimality,
            relaxation,
        })
    }
}

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

impl From<Solution> for crate::v1::Solution {
    fn from(solution: Solution) -> Self {
        let state = solution.state().clone();
        let objective = *solution.objective();
        let evaluated_constraints = solution
            .evaluated_constraints()
            .iter()
            .map(|ec| ec.clone().into())
            .collect();
        let decision_variables: Vec<crate::v1::DecisionVariable> = solution
            .decision_variables()
            .iter()
            .map(|dv| {
                let dv_converted = dv.to_decision_variable().unwrap();
                dv_converted.into()
            })
            .collect();
        let feasible = *solution.feasible();
        let feasible_relaxed = Some(*solution.feasible_relaxed());
        let optimality = (*solution.optimality()).into();
        let relaxation = (*solution.relaxation()).into();
        // For backward compatibility, set feasible_unrelaxed to the same value as feasible
        let feasible_unrelaxed = feasible;

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
        }
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
    fn test_solution_parse() {
        let v1_solution = v1::Solution {
            state: Some(v1::State {
                entries: [(1, 2.0), (2, 3.0)].iter().cloned().collect(),
            }),
            objective: 42.5,
            evaluated_constraints: vec![v1::EvaluatedConstraint {
                id: 1,
                equality: v1::Equality::EqualToZero as i32,
                evaluated_value: 0.1,
                dual_variable: Some(1.5),
                name: Some("test_constraint".to_string()),
                ..Default::default()
            }],
            decision_variables: vec![v1::DecisionVariable {
                id: 1,
                name: Some("x1".to_string()),
                kind: v1::decision_variable::Kind::Continuous as i32,
                ..Default::default()
            }],
            feasible: true,
            feasible_relaxed: Some(true),
            optimality: v1::Optimality::Optimal as i32,
            relaxation: v1::Relaxation::Unspecified as i32,
            ..Default::default()
        };

        let parsed: Solution = v1_solution.parse(&()).unwrap();

        assert_eq!(parsed.objective(), &42.5);
        assert_eq!(parsed.feasible(), &true);
        assert_eq!(parsed.feasible_relaxed(), &true);
        assert_eq!(*parsed.optimality(), v1::Optimality::Optimal);
        assert_eq!(*parsed.relaxation(), v1::Relaxation::Unspecified);
        assert_eq!(parsed.evaluated_constraints().len(), 1);
        assert_eq!(parsed.decision_variables().len(), 1);

        // Test round-trip conversion
        let v1_converted: v1::Solution = parsed.into();
        assert_eq!(v1_converted.objective, 42.5);
        assert_eq!(v1_converted.feasible, true);
        assert_eq!(v1_converted.feasible_relaxed, Some(true));
    }

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
    fn test_unknown_enum_value_error() {
        // Test with an invalid optimality value
        let v1_solution = v1::Solution {
            optimality: 99, // Unknown enum value
            relaxation: v1::Relaxation::Unspecified as i32,
            feasible: true,
            ..Default::default()
        };

        let result: Result<Solution, ParseError> = v1_solution.parse(&());
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error
            .to_string()
            .contains("Unknown or unsupported enum value 99 for ommx.v1.Optimality"));

        // Test with an invalid relaxation value
        let v1_solution2 = v1::Solution {
            optimality: v1::Optimality::Optimal as i32,
            relaxation: 123, // Unknown enum value
            feasible: true,
            ..Default::default()
        };

        let result2: Result<Solution, ParseError> = v1_solution2.parse(&());
        assert!(result2.is_err());

        let error2 = result2.unwrap_err();
        assert!(error2
            .to_string()
            .contains("Unknown or unsupported enum value 123 for ommx.v1.Relaxation"));
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
