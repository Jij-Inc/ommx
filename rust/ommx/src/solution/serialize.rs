use super::*;
use crate::{message_io, v1, v2, Message, Parse};
use anyhow::Result;

impl Solution {
    /// Serialize using the lossy v1 format.
    ///
    /// Native special constraints are omitted. Feasibility is recomputed from
    /// retained regular constraints and variable values at [`ATol::default`],
    /// as on v1 import. Use the same default on write and read, or use
    /// [`Self::to_v2_bytes`] to preserve the original tolerance and constraints.
    pub fn to_v1_bytes(&self) -> Vec<u8> {
        let v1_solution = v1::Solution::from(self.clone());
        v1_solution.encode_to_vec()
    }

    pub fn to_v2_bytes(&self) -> Vec<u8> {
        let v2_solution = v2::Solution::from(self.clone());
        v2_solution.encode_to_vec()
    }

    pub fn from_v1_bytes(bytes: &[u8]) -> Result<Self> {
        let inner = message_io::decode::<v1::Solution>(bytes, "ommx.v1.Solution")?;
        Ok(Parse::parse(inner, &())?)
    }

    pub fn from_v2_bytes(bytes: &[u8]) -> Result<Self> {
        let inner = message_io::decode::<v2::Solution>(bytes, "ommx.v2.Solution")?;
        Ok(Parse::parse(inner, &())?)
    }
}

impl From<Solution> for v2::Solution {
    fn from(value: Solution) -> Self {
        let required_features = crate::v2_io::required_features(
            !value.evaluated_indicator_constraints.is_empty(),
            !value.evaluated_one_hot_constraints.is_empty(),
            !value.evaluated_sos1_constraints.is_empty(),
        );
        let feasible = value.feasible();
        let feasible_relaxed = value.feasible_relaxed();

        let Solution {
            objective,
            evaluated_constraints,
            evaluated_indicator_constraints,
            evaluated_one_hot_constraints,
            evaluated_sos1_constraints,
            evaluated_named_functions,
            decision_variables,
            optimality,
            relaxation,
            sense,
            feasibility_atol,
            metadata,
            annotations,
        } = value;

        Self {
            required_features,
            objective,
            decision_variables: Some(decision_variables.into()),
            evaluated_regular_constraints: Some(evaluated_constraints.into_v2(feasibility_atol)),
            feasible,
            optimality: optimality.into(),
            relaxation: relaxation.into(),
            feasible_relaxed: Some(feasible_relaxed),
            sense: sense.map(Into::into).unwrap_or_default(),
            evaluated_named_functions: Some(evaluated_named_functions.into()),
            metadata,
            annotations: crate::v2_io::extension_annotations_to_v2_map(annotations),
            evaluated_indicator_constraints: Some(
                evaluated_indicator_constraints.into_v2(feasibility_atol),
            ),
            evaluated_one_hot_constraints: Some(
                evaluated_one_hot_constraints.into_v2(feasibility_atol),
            ),
            evaluated_sos1_constraints: Some(evaluated_sos1_constraints.into_v2(feasibility_atol)),
            feasibility_atol: Some(feasibility_atol.into_inner()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Bound, Coefficient, Constraint, DecisionVariable, Evaluate, Function, Instance,
        RemovedReason, SampleID, SampleSet, Sampled,
    };
    use proptest::prelude::*;

    #[test]
    fn v1_roundtrip_normalizes_constraint_and_variable_feasibility() {
        let default_atol = ATol::default();
        let sample_id = SampleID::from(7);
        for (factor, value_factor, expected) in [(100.0, 10.0, false), (0.01, 0.1, true)] {
            let atol = ATol::new(default_atol.into_inner() * factor).unwrap();
            let value = default_atol.into_inner() * value_factor;
            for source in ["regular", "removed", "variable"] {
                let constraint = Constraint::equal_to_zero(Function::from(
                    Coefficient::try_from(value).unwrap(),
                ));
                let mut builder = Instance::builder()
                    .sense(Sense::Minimize)
                    .objective(Function::Zero)
                    .decision_variables(BTreeMap::new())
                    .constraints(BTreeMap::new());
                let mut state = v1::State::default();
                match source {
                    "regular" => {
                        builder = builder.constraints(BTreeMap::from([(0.into(), constraint)]));
                    }
                    "removed" => {
                        builder = builder.removed_constraints(BTreeMap::from([(
                            0.into(),
                            (
                                constraint,
                                RemovedReason {
                                    reason: "test relaxation".into(),
                                    parameters: Default::default(),
                                },
                            ),
                        )]));
                    }
                    "variable" => {
                        let variable = DecisionVariable::continuous()
                            .with_bound(Bound::new(-1.0, 0.0).unwrap(), default_atol)
                            .unwrap();
                        builder =
                            builder.decision_variables(BTreeMap::from([(0.into(), variable)]));
                        state.entries.insert(0, value);
                    }
                    _ => unreachable!(),
                }
                let instance = builder.build().unwrap();
                let solution = instance.evaluate(&state, atol).unwrap();
                let samples = instance
                    .evaluate_samples(&Sampled::from((sample_id, state)), atol)
                    .unwrap();
                assert_eq!(solution.feasible(), !expected, "{source}");
                assert_eq!(
                    samples.feasible_ids().contains(&sample_id),
                    !expected,
                    "{source}"
                );

                let wire: v1::SampleSet = samples.clone().into();
                if source != "variable" {
                    assert_eq!(wire.constraints[0].feasible[&7], expected);
                }
                let restored_samples = SampleSet::from_v1_bytes(&samples.to_v1_bytes()).unwrap();
                assert_eq!(
                    restored_samples.feasible_ids().contains(&sample_id),
                    expected
                );
                assert_eq!(
                    restored_samples.feasible_relaxed_ids().contains(&sample_id),
                    source == "removed" || expected
                );
                let restored = Solution::from_v1_bytes(&solution.to_v1_bytes()).unwrap();
                for result in [restored, restored_samples.get(sample_id).unwrap()] {
                    assert_eq!(result.feasibility_atol(), default_atol);
                    assert_eq!(result.feasible(), expected, "{source}");
                    assert_eq!(
                        result.feasible_relaxed(),
                        source == "removed" || expected,
                        "{source}"
                    );
                    assert_eq!(result.state(), solution.state());
                    assert_eq!(
                        result.evaluated_constraints(),
                        solution.evaluated_constraints()
                    );
                }

                // v2 retains the original tolerance and its judgments.
                let restored = Solution::from_v2_bytes(&solution.to_v2_bytes()).unwrap();
                let restored_samples = SampleSet::from_v2_bytes(&samples.to_v2_bytes()).unwrap();
                assert_eq!(restored, solution);
                assert_eq!(restored_samples.feasibility_atol(), atol);
                assert_eq!(restored_samples.feasible_ids(), samples.feasible_ids());
                assert_eq!(
                    restored_samples.feasible_relaxed_ids(),
                    samples.feasible_relaxed_ids()
                );
            }
        }
    }

    #[test]
    fn v1_solution_retains_infeasible_saved_variable_values() {
        for variable in [DecisionVariable::binary(), DecisionVariable::integer()] {
            let instance = Instance::builder()
                .sense(Sense::Minimize)
                .objective(Function::Zero)
                .decision_variables(BTreeMap::from([(0.into(), variable)]))
                .constraints(BTreeMap::new())
                .build()
                .unwrap();
            let solution = instance
                .evaluate(
                    &v1::State {
                        entries: [(0, 0.5)].into(),
                    },
                    ATol::default(),
                )
                .unwrap();
            assert!(!solution.feasible());
            for with_state in [true, false] {
                let mut wire = v1::Solution::from(solution.clone());
                if !with_state {
                    wire.state = None;
                }
                let restored = Solution::from_v1_bytes(&wire.encode_to_vec()).unwrap();
                assert_eq!(restored, solution);
            }
            for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                let mut wire = v1::Solution::from(solution.clone());
                wire.state = None;
                wire.decision_variables[0].substituted_value = Some(value);
                assert!(Solution::from_v1_bytes(&wire.encode_to_vec()).is_err());
            }
        }
    }

    #[test]
    fn v1_roundtrip_excludes_omitted_special_constraints_from_feasibility() {
        for family in ["indicator", "one-hot", "sos1"] {
            let variables: crate::VariableIDSet = [0.into(), 1.into()].into_iter().collect();
            let mut builder = Instance::builder()
                .sense(Sense::Minimize)
                .objective(Function::Zero)
                .constraints(BTreeMap::new())
                .decision_variables(
                    variables
                        .iter()
                        .map(|&id| (id, DecisionVariable::binary()))
                        .collect(),
                );
            match family {
                "indicator" => {
                    builder = builder.indicator_constraints(BTreeMap::from([(
                        0.into(),
                        crate::IndicatorConstraint::new(
                            0.into(),
                            crate::Equality::EqualToZero,
                            Function::from(crate::coeff!(1.0)),
                        ),
                    )]));
                }
                "one-hot" => {
                    builder = builder.one_hot_constraints(BTreeMap::from([(
                        0.into(),
                        crate::OneHotConstraint::new(variables).unwrap(),
                    )]));
                }
                "sos1" => {
                    builder = builder.sos1_constraints(BTreeMap::from([(
                        0.into(),
                        crate::Sos1Constraint::new(variables).unwrap(),
                    )]));
                }
                _ => unreachable!(),
            }
            let instance = builder.build().unwrap();
            let state = v1::State {
                entries: [(0, 1.0), (1, 1.0)].into(),
            };
            let solution = instance.evaluate(&state, ATol::default()).unwrap();
            let samples = instance
                .evaluate_samples(&Sampled::from((7.into(), state)), ATol::default())
                .unwrap();
            assert!(!solution.feasible(), "{family}");
            assert!(samples.feasible_ids().is_empty(), "{family}");
            let restored = Solution::from_v1_bytes(&solution.to_v1_bytes()).unwrap();
            let restored_samples = SampleSet::from_v1_bytes(&samples.to_v1_bytes()).unwrap();
            for result in [restored, restored_samples.get(7.into()).unwrap()] {
                assert!(result.feasible(), "{family}");
                assert!(result.feasible_relaxed(), "{family}");
                assert_eq!(result.total_violation(), 0.0);
            }
        }
    }

    proptest! {
        #[test]
        fn v1_roundtrip_preserves_the_retained_projection(
            (instance, state) in Instance::arbitrary().prop_flat_map(|instance| {
                let state = instance.arbitrary_state();
                (Just(instance), state)
            }),
            factor in prop_oneof![Just(0.01), Just(100.0)],
        ) {
            let atol = ATol::new(ATol::default().into_inner() * factor).unwrap();
            let solution = match instance.evaluate(&state, atol) {
                Ok(solution) => solution,
                Err(error) => {
                    // Composed functions may be undefined at a generated state.
                    prop_assert!(error.is::<crate::FunctionEvaluationError>(), "{error:#}");
                    return Ok(());
                }
            };
            let samples = instance.evaluate_samples(&Sampled::from((7.into(), state)), atol).unwrap();
            let restored = Solution::from_v1_bytes(&solution.to_v1_bytes()).unwrap();
            let restored_samples = SampleSet::from_v1_bytes(&samples.to_v1_bytes()).unwrap();
            // v1 also omits provenance references to native special constraints.
            // Check the representable projection, including its wire idempotence.
            prop_assert_eq!(v1::Solution::from(restored.clone()), v1::Solution::from(solution.clone()));
            prop_assert_eq!(v1::SampleSet::from(restored_samples.clone()), v1::SampleSet::from(samples));
            for result in [&restored, &restored_samples.get(7.into()).unwrap()] {
                prop_assert_eq!(result.state(), solution.state());
                prop_assert_eq!(result.evaluated_constraints().inner(), solution.evaluated_constraints().inner());
                prop_assert_eq!(result.variable_labels(), solution.variable_labels());
                prop_assert_eq!(result.feasibility_atol(), ATol::default());
                prop_assert!(result.evaluated_indicator_constraints().is_empty());
                prop_assert!(result.evaluated_one_hot_constraints().is_empty());
                prop_assert!(result.evaluated_sos1_constraints().is_empty());
            }
            prop_assert_eq!(restored_samples.feasible_ids().contains(&7.into()), restored.feasible());
            prop_assert_eq!(restored_samples.feasible_relaxed_ids().contains(&7.into()), restored.feasible_relaxed());
        }
    }
}
