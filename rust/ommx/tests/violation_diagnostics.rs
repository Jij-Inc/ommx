use ommx::{
    v1::State, ATol, ConstraintRole, ConstraintType, DecisionVariable, Evaluate, Function,
    Instance, OneHotConstraint, OneHotViolationDiagnostics, SampleID, SampleSet, Sampled, Sense,
    Solution, Sos1Constraint,
};
use std::collections::BTreeMap;

// Compile these helpers as a downstream crate: no internal host or dispatch
// bound may be needed, even when the constraint family is not concrete yet.
fn solution_diagnostics<C: ConstraintType>(
    solution: &Solution,
    role: ConstraintRole,
) -> BTreeMap<C::ID, C::ViolationDiagnostics> {
    solution.violation_diagnostics::<C>(role)
}

fn sample_diagnostics<C: ConstraintType>(
    samples: &SampleSet,
    sample_id: SampleID,
    role: ConstraintRole,
) -> ommx::Result<BTreeMap<C::ID, C::ViolationDiagnostics>> {
    samples.violation_diagnostics::<C>(sample_id, role)
}

fn model(n: usize) -> Instance {
    let members = (0..n as u64).map(Into::into).collect();
    Instance::builder()
        .sense(Sense::Minimize)
        .objective(Function::Zero)
        .decision_variables(
            (0..n as u64)
                .map(|id| (id.into(), DecisionVariable::binary()))
                .collect(),
        )
        .active_constraints(BTreeMap::new())
        .active_one_hot_constraints([(0.into(), OneHotConstraint::new(members).unwrap())].into())
        .active_sos1_constraints(
            [(
                1.into(),
                Sos1Constraint::new((0..n as u64).map(Into::into).collect()).unwrap(),
            )]
            .into(),
        )
        .build()
        .unwrap()
}

fn state(values: &[f64]) -> State {
    State::from_iter(
        values
            .iter()
            .enumerate()
            .map(|(id, &value)| (id as u64, value)),
    )
}

#[test]
fn one_hot_variants_support_explicit_field_patterns_from_downstream() {
    let model = model(3);
    for (case, values) in [[0.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.875, 0.25, 0.0]]
        .into_iter()
        .enumerate()
    {
        let solution = model.evaluate(&state(&values), ATol::default()).unwrap();
        let diagnostic = solution
            .violation_diagnostics::<OneHotConstraint>(ConstraintRole::EvaluationOnly)
            .remove(&0.into())
            .unwrap();
        // Known variants have fixed fields, but future variants need a fallback.
        let retained = match diagnostic {
            OneHotViolationDiagnostics::MissingSelection { values } => {
                assert_eq!(case, 0);
                values
            }
            OneHotViolationDiagnostics::MultipleSelections {
                selected_variables,
                values,
            } => {
                assert_eq!(case, 1);
                assert_eq!(selected_variables, [0.into(), 1.into()].into());
                values
            }
            OneHotViolationDiagnostics::ValueErrors {
                target_variable,
                errors,
                values,
            } => {
                assert_eq!(case, 2);
                assert_eq!(target_variable, 0.into());
                assert_eq!(errors, [(0.into(), 0.125), (1.into(), 0.25)].into());
                values
            }
            _ => panic!("unexpected diagnostic variant"),
        };
        assert_eq!(
            retained,
            values
                .into_iter()
                .enumerate()
                .map(|(id, x)| ((id as u64).into(), x))
                .collect()
        );
    }
}

#[test]
fn one_hot_target_breaks_signed_zero_ties_by_variable_id() {
    // Result builders accept finite solver values without input canonicalization.
    let values = [0.0, -0.0, -2.0];
    let states = Sampled::from((7.into(), state(&values)));
    let constraint = OneHotConstraint::new([0.into(), 1.into(), 2.into()].into()).unwrap();
    assert_eq!(
        constraint
            .evaluate(&state(&values), ATol::new(3.0).unwrap())
            .unwrap()
            .stage
            .selected_variable,
        Some(1.into())
    );
    let samples = SampleSet::builder()
        .objectives(Sampled::from((7.into(), 0.0)))
        .constraints(BTreeMap::new())
        .one_hot_constraints(
            [(
                0.into(),
                constraint
                    .evaluate_samples(&states, ATol::default())
                    .unwrap(),
            )]
            .into(),
        )
        .decision_variables(
            values
                .into_iter()
                .enumerate()
                .map(|(id, value)| {
                    let id = (id as u64).into();
                    (
                        id,
                        ommx::SampledDecisionVariable::new(
                            id,
                            DecisionVariable::binary(),
                            Sampled::from((7.into(), value)),
                        )
                        .unwrap(),
                    )
                })
                .collect(),
        )
        .sense(Sense::Minimize)
        .build()
        .unwrap();
    assert!(samples.decision_variables()[&1.into()]
        .samples()
        .get(7.into())
        .unwrap()
        .is_sign_negative());
    let restored = SampleSet::from_v2_bytes(&samples.to_v2_bytes()).unwrap();
    for samples in [&samples, &restored] {
        let solution = samples.get(7.into()).unwrap();
        let direct = Solution::from_v2_bytes(&solution.to_v2_bytes()).unwrap();
        for solution in [&solution, &direct] {
            let diagnostics =
                solution.violation_diagnostics::<OneHotConstraint>(ConstraintRole::EvaluationOnly);
            let OneHotViolationDiagnostics::ValueErrors {
                target_variable,
                errors,
                values,
            } = &diagnostics[&0.into()]
            else {
                panic!("expected value errors");
            };
            assert_eq!(*target_variable, 1.into());
            assert_eq!(*errors, [(1.into(), 1.0), (2.into(), 2.0)].into());
            // Protobuf may normalize signed zero. Diagnostics preserve this
            // host's saved values and choose the same repair either way.
            assert_eq!(
                values[&1.into()].to_bits(),
                solution.decision_variables()[&1.into()].value().to_bits()
            );
            assert_eq!(
                samples
                    .violation_diagnostics::<OneHotConstraint>(
                        7.into(),
                        ConstraintRole::EvaluationOnly
                    )
                    .unwrap(),
                diagnostics
            );
        }
    }
}

#[test]
fn every_family_filters_roles_and_survives_storage_and_extraction() {
    use ommx::{Constraint, Equality, IndicatorConstraint, IndicatorTriggerValue};
    let regular = Constraint::equal_to_zero(ommx::linear!(0).into());
    let indicator = IndicatorConstraint::new(
        2.into(),
        Equality::EqualToZero,
        ommx::linear!(0).into(),
        IndicatorTriggerValue::One,
    );
    let one_hot = OneHotConstraint::new([0.into(), 1.into()].into()).unwrap();
    let sos1 = Sos1Constraint::new([0.into(), 1.into()].into()).unwrap();
    let mut model = Instance::builder()
        .sense(Sense::Minimize)
        .objective(Function::Zero)
        .decision_variables(
            [
                (0.into(), DecisionVariable::binary()),
                (1.into(), DecisionVariable::binary()),
                (2.into(), DecisionVariable::binary()),
            ]
            .into(),
        )
        .active_constraints((0..3).map(|id| (id.into(), regular.clone())).collect())
        .active_indicator_constraints((10..13).map(|id| (id.into(), indicator.clone())).collect())
        .active_one_hot_constraints((20..23).map(|id| (id.into(), one_hot.clone())).collect())
        .active_sos1_constraints((30..33).map(|id| (id.into(), sos1.clone())).collect())
        .build()
        .unwrap();
    for (offset, skip) in [(1, false), (2, true)] {
        model
            .deactivate_constraint(offset.into(), "retained".into(), [], skip)
            .unwrap();
        model
            .deactivate_indicator_constraint((10 + offset).into(), "retained".into(), [], skip)
            .unwrap();
        model
            .deactivate_one_hot_constraint((20 + offset).into(), "retained".into(), [], skip)
            .unwrap();
        model
            .deactivate_sos1_constraint((30 + offset).into(), "retained".into(), [], skip)
            .unwrap();
    }
    let states = Sampled::new(
        [vec![2.into(), 7.into()], vec![13.into()]],
        [state(&[1.0, 1.0, 1.0]), state(&[0.0, 1.0, 1.0])],
    )
    .unwrap();
    let samples = model.evaluate_samples(&states, ATol::default()).unwrap();
    let restored = SampleSet::from_v2_bytes(&samples.to_v2_bytes()).unwrap();
    for samples in [&samples, &restored] {
        for sid in [2.into(), 7.into(), 13.into()] {
            let solution = samples.get(sid).unwrap();
            let restored = Solution::from_v2_bytes(&solution.to_v2_bytes()).unwrap();
            for solution in [&solution, &restored] {
                for (role, count) in [
                    (ConstraintRole::Active, 1),
                    (ConstraintRole::FeasibilityCheck, 2),
                    (ConstraintRole::EvaluationOnly, 3),
                ] {
                    macro_rules! check {
                        ($family:ty, $base:expr) => {{
                            let expected = if sid == 13.into() {
                                Vec::new()
                            } else {
                                (0..count)
                                    .map(|offset| ($base + offset).into())
                                    .collect::<Vec<_>>()
                            };
                            let diagnostics = solution_diagnostics::<$family>(&solution, role);
                            assert_eq!(diagnostics.keys().copied().collect::<Vec<_>>(), expected);
                            assert_eq!(
                                diagnostics,
                                sample_diagnostics::<$family>(&samples, sid, role).unwrap()
                            );
                        }};
                    }
                    check!(Constraint, 0);
                    check!(IndicatorConstraint, 10);
                    check!(OneHotConstraint, 20);
                    check!(Sos1Constraint, 30);
                }
            }
        }
    }
}
