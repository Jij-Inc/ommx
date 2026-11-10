use ommx::{
    coeff, linear, v1::State, v2, ATol, Bound, ConstraintActiveness, DecisionVariable, Equality,
    Evaluate, EvaluatedConstraintBehavior, Function, IndicatorConstraint, IndicatorConstraintID,
    Instance, InstanceClass, InstanceClassClause, InstanceClassMismatch, Kind, Message,
    ParametricInstance, PolynomialRequirement, Propagate, PropagateOutcome, SampleSet, Sampled,
    Sense, Solution,
};
use std::collections::{BTreeMap, BTreeSet};

fn instance(zero: bool, equality: Equality) -> Instance {
    Instance::builder()
        .sense(Sense::Minimize)
        .objective(Function::Zero)
        .decision_variables(BTreeMap::from([
            (0.into(), DecisionVariable::binary()),
            (
                1.into(),
                DecisionVariable::continuous()
                    .with_bound(Bound::new(-5.0, 5.0).unwrap(), ATol::default())
                    .unwrap(),
            ),
        ]))
        .active_constraints(BTreeMap::new())
        .active_indicator_constraints(BTreeMap::from([(
            7.into(),
            IndicatorConstraint::new(
                0.into(),
                equality,
                Function::from((linear!(1) - coeff!(2.0)).unwrap()),
                if zero {
                    ommx::IndicatorTriggerValue::Zero
                } else {
                    ommx::IndicatorTriggerValue::One
                },
            ),
        )]))
        .build()
        .unwrap()
}

#[test]
fn skipped_zero_activated_indicator_preserves_both_policies() {
    let mut model = instance(true, Equality::EqualToZero);
    model
        .deactivate_indicator_constraint(7.into(), "diagnostic".into(), [], true)
        .unwrap();
    let parametric = ParametricInstance::from(model.clone());
    let state = State::from_iter([(0, 0.0), (1, 3.0)]);
    let solution = model.evaluate(&state, ATol::default()).unwrap();
    let states = Sampled::new_no_dedup(
        [
            (0.into(), state),
            (1.into(), State::from_iter([(0, 1.0), (1, 3.0)])),
        ]
        .into_iter(),
    );
    let samples = model.evaluate_samples(&states, ATol::default()).unwrap();
    let row = &solution.evaluated_indicator_constraints().inner()[&7.into()];
    assert_eq!(row.trigger_value, ommx::IndicatorTriggerValue::Zero);
    assert!(row.stage.indicator_on);
    assert_eq!(row.violation(), 1.0);
    assert!(solution.feasible());
    assert!(solution.feasible_relaxed());
    assert_eq!(
        solution.total_violation(ConstraintActiveness::InactiveChecked),
        0.0
    );
    assert_eq!(
        solution.total_violation(ConstraintActiveness::InactiveSkipped),
        1.0
    );
    assert!(samples.feasible().values().all(|feasible| *feasible));
    assert_eq!(
        samples.indicator_constraints().inner()[&7.into()]
            .stage
            .indicator_on,
        BTreeMap::from([(0.into(), true), (1.into(), false)])
    );

    for features in [
        v2::Instance::from(model.clone()).required_features,
        v2::ParametricInstance::from(parametric.clone()).required_features,
        v2::Solution::from(solution.clone()).required_features,
        v2::SampleSet::from(samples.clone()).required_features,
    ] {
        assert!(features.contains(&(v2::Feature::IndicatorTriggerValue as i32)));
        assert!(features.contains(&(v2::Feature::ConstraintSkipFeasibilityCheck as i32)));
    }
    assert_eq!(
        Instance::from_v2_bytes(&model.to_v2_bytes()).unwrap(),
        model
    );
    assert_eq!(
        ParametricInstance::from_v2_bytes(&parametric.to_v2_bytes()).unwrap(),
        parametric
    );
    assert_eq!(
        Solution::from_v2_bytes(&solution.to_v2_bytes()).unwrap(),
        solution
    );
    let restored = SampleSet::from_v2_bytes(&samples.to_v2_bytes()).unwrap();
    assert_eq!(restored.to_v2_bytes(), samples.to_v2_bytes());
    assert_eq!(restored.get(0.into()).unwrap(), solution);
    assert!(model.to_v1_bytes().is_err());
    assert!(parametric.to_v1_bytes().is_err());
    assert!(solution.to_v1_bytes().is_err());
    assert!(samples.to_v1_bytes().is_err());
}

#[test]
fn activation_evaluation_propagation_and_bigm() {
    for zero in [false, true] {
        for equality in [Equality::EqualToZero, Equality::LessThanOrEqualToZero] {
            let source = instance(zero, equality);
            let indicator = source
                .active_indicator_constraints()
                .get(&7.into())
                .unwrap();
            let mut lowered = source.clone();
            lowered
                .convert_all_indicators_to_constraints(ATol::default())
                .unwrap();
            assert_eq!(
                lowered.inactive_indicator_constraints()[&7.into()]
                    .0
                    .trigger_value,
                if zero {
                    ommx::IndicatorTriggerValue::Zero
                } else {
                    ommx::IndicatorTriggerValue::One
                }
            );
            for z in [0.0, 1.0] {
                let indicator_on = z == if zero { 0.0 } else { 1.0 };
                let fixed = State::from_iter([(0, z)]);
                let (outcome, _) = indicator
                    .clone()
                    .propagate(&fixed, ATol::default())
                    .unwrap();
                assert_eq!(
                    matches!(outcome, PropagateOutcome::Transformed { .. }),
                    indicator_on
                );
                assert_eq!(
                    matches!(outcome, PropagateOutcome::Consumed(_)),
                    !indicator_on
                );
                for x in [-5.0, 0.0, 2.0, 5.0] {
                    let state = State::from_iter([(0, z), (1, x)]);
                    let expected = !indicator_on
                        || if equality == Equality::EqualToZero {
                            x == 2.0
                        } else {
                            x <= 2.0
                        };
                    let result = source.evaluate(&state, ATol::default()).unwrap();
                    assert_eq!(result.feasible(), expected);
                    assert_eq!(
                        lowered
                            .evaluate(&state, ATol::default())
                            .unwrap()
                            .feasible(),
                        expected
                    );
                    // Check generated rows independently of the retained original indicator.
                    assert_eq!(
                        lowered
                            .evaluate(&state, ATol::default())
                            .unwrap()
                            .feasible_relaxed(),
                        expected
                    );
                    let row = &result.evaluated_indicator_constraints().inner()[&7.into()];
                    assert_eq!(u8::from(row.trigger_value), u8::from(!zero));
                    assert_eq!(row.stage.indicator_on, indicator_on);
                    assert_eq!(row.is_feasible(ATol::default()), expected);
                    assert_eq!(
                        Solution::from_v2_bytes(&result.to_v2_bytes()).unwrap(),
                        result
                    );
                }
            }
        }
    }
}

#[test]
fn activation_tolerance_and_invalid_values() {
    let atol = ATol::new(1e-6).unwrap();
    for zero in [false, true] {
        let model = instance(zero, Equality::EqualToZero);
        for (value, indicator_on) in [(0.0, zero), (5e-7, zero), (1.0 - 5e-7, !zero), (1.0, !zero)]
        {
            let state = State::from_iter([(0, value), (1, 3.0)]);
            let solution = model.evaluate(&state, atol).unwrap();
            assert_eq!(solution.feasible(), !indicator_on);
            assert_eq!(
                Solution::from_v2_bytes(&solution.to_v2_bytes()).unwrap(),
                solution
            );
        }
        for value in [0.5, f64::NAN, f64::INFINITY] {
            assert!(model
                .evaluate(&State::from_iter([(0, value), (1, 3.0)]), atol)
                .is_err());
        }
    }
}

#[test]
fn all_wire_roots_declare_activation_feature_including_inactive_rows() {
    let feature = v2::Feature::IndicatorTriggerValue as i32;
    for zero in [false, true] {
        let model = instance(zero, Equality::EqualToZero);
        let proto: v2::Instance = model.clone().into();
        assert_eq!(proto.required_features.contains(&feature), zero);
        assert_eq!(
            Instance::from_v2_bytes(&model.to_v2_bytes()).unwrap(),
            model
        );
        let parametric: ParametricInstance = model.clone().into();
        let proto: v2::ParametricInstance = parametric.clone().into();
        assert_eq!(proto.required_features.contains(&feature), zero);
        assert_eq!(
            ParametricInstance::from_v2_bytes(&parametric.to_v2_bytes()).unwrap(),
            parametric
        );
        let states = Sampled::new_no_dedup(
            [
                (0.into(), State::from_iter([(0, 0.0), (1, 3.0)])),
                (1.into(), State::from_iter([(0, 1.0), (1, 3.0)])),
            ]
            .into_iter(),
        );
        let samples = model.evaluate_samples(&states, ATol::default()).unwrap();
        let proto: v2::SampleSet = samples.clone().into();
        assert_eq!(proto.required_features.contains(&feature), zero);
        assert_eq!(
            SampleSet::from_v2_bytes(&samples.to_v2_bytes())
                .unwrap()
                .to_v2_bytes(),
            samples.to_v2_bytes()
        );
        for (_, state) in states.iter() {
            let solution = model.evaluate(state, ATol::default()).unwrap();
            let proto: v2::Solution = solution.into();
            assert_eq!(proto.required_features.contains(&feature), zero);
        }
        let mut lowered = model;
        lowered
            .convert_all_indicators_to_constraints(ATol::default())
            .unwrap();
        let proto: v2::Instance = lowered.clone().into();
        assert_eq!(proto.required_features.contains(&feature), zero);
        assert_eq!(
            Instance::from_v2_bytes(&lowered.to_v2_bytes()).unwrap(),
            lowered
        );
        let parametric: ParametricInstance = lowered.into();
        let proto: v2::ParametricInstance = parametric.clone().into();
        assert_eq!(proto.required_features.contains(&feature), zero);
        assert_eq!(
            ParametricInstance::from_v2_bytes(&parametric.to_v2_bytes()).unwrap(),
            parametric
        );
    }
    // An absent trigger value uses the default value one.
    let row = v2::IndicatorConstraint::decode(&b""[..]).unwrap();
    assert_eq!(row.trigger_value, None);
}

fn class(values: &[u8], conditions: &[(Equality, u32)]) -> InstanceClass {
    let mut clause = InstanceClassClause::new(
        "activation",
        BTreeSet::from([Kind::Binary, Kind::Continuous]),
        PolynomialRequirement::at_most(1),
        BTreeSet::from([Sense::Minimize]),
    )
    .with_allowed_indicator_trigger_values(values.iter().copied().collect())
    .unwrap();
    for &(relation, degree) in conditions {
        clause = clause.with_indicator_constraint(relation, PolynomialRequirement::at_most(degree));
    }
    clause.into()
}

#[test]
fn class_checks_activation_and_body_independently_without_mutation() {
    let eq = Equality::EqualToZero;
    let le = Equality::LessThanOrEqualToZero;
    for zero in [false, true] {
        let model = instance(zero, eq);
        let value = u8::from(!zero);
        let before = model.to_v2_bytes();
        assert!(class(&[value], &[(eq, 1)]).contains(&model));
        assert!(class(&[0, 1], &[(eq, 1)]).contains(&model));
        let report = class(&[1 - value], &[(eq, 1)]).check_membership(&model);
        assert!(matches!(&report.clause_reports()[0].mismatches()[0],
            InstanceClassMismatch::IndicatorTriggerNotAllowed { trigger_value, constraint_ids, .. }
            if *trigger_value == value && constraint_ids == &BTreeSet::from([IndicatorConstraintID::from(7)])));
        assert!(!class(&[0, 1], &[(le, 2)]).contains(&model));
        assert!(!class(&[0, 1], &[(eq, 0)]).contains(&model));
        assert!(!class(&[], &[(eq, 1)]).contains(&model));
        assert!(!class(&[0, 1], &[]).contains(&model));
        // An invalid activation does not suppress the independent body mismatch.
        let report = class(&[1 - value], &[(eq, 0)]).check_membership(&model);
        assert_eq!(report.clause_reports()[0].mismatches().len(), 2);
        assert!(matches!(
            report.clause_reports()[0].mismatches()[1],
            InstanceClassMismatch::IndicatorBodyDegreeExceedsBound { .. }
        ));
        assert_eq!(before, model.to_v2_bytes());
    }
    let mut mixed = instance(false, eq);
    mixed
        .add_indicator_constraint(
            IndicatorConstraint::new(
                0.into(),
                le,
                Function::Zero,
                ommx::IndicatorTriggerValue::Zero,
            ),
            Default::default(),
        )
        .unwrap();
    assert!(!class(&[1], &[(eq, 1), (le, 1)]).contains(&mixed));
    assert!(!class(&[0], &[(eq, 1), (le, 1)]).contains(&mixed));
    assert!(class(&[0, 1], &[(eq, 1), (le, 1)]).contains(&mixed));
    let clause = InstanceClassClause::new(
        "default",
        BTreeSet::new(),
        PolynomialRequirement::at_most(1),
        BTreeSet::from([Sense::Minimize]),
    );
    assert_eq!(
        clause.allowed_indicator_trigger_values(),
        &BTreeSet::from([1])
    );
    assert!(clause
        .with_allowed_indicator_trigger_values(BTreeSet::from([0, 2]))
        .is_err());
}

#[test]
fn constant_body_lowering_and_wire_activation_consistency() {
    for zero in [false, true] {
        for equality in [Equality::EqualToZero, Equality::LessThanOrEqualToZero] {
            for residual in [-1.0, 0.0, 1.0] {
                let mut proto: v2::Instance = instance(zero, equality).into();
                proto
                    .indicator_constraints
                    .as_mut()
                    .unwrap()
                    .active
                    .get_mut(&7)
                    .unwrap()
                    .function = Some(
                    if residual == 0.0 {
                        Function::Zero
                    } else {
                        Function::from(ommx::Coefficient::try_from(residual).unwrap())
                    }
                    .into(),
                );
                let source = Instance::from_v2_bytes(&proto.encode_to_vec()).unwrap();
                let mut lowered = source.clone();
                lowered
                    .convert_all_indicators_to_constraints(ATol::default())
                    .unwrap();
                for z in [0.0, 1.0] {
                    let state = State::from_iter([(0, z), (1, 0.0)]);
                    let expected = z != if zero { 0.0 } else { 1.0 }
                        || if equality == Equality::EqualToZero {
                            residual == 0.0
                        } else {
                            residual <= 0.0
                        };
                    assert_eq!(
                        lowered
                            .evaluate(&state, ATol::default())
                            .unwrap()
                            .feasible(),
                        expected
                    );
                    assert_eq!(
                        lowered
                            .evaluate(&state, ATol::default())
                            .unwrap()
                            .feasible_relaxed(),
                        expected
                    );
                }
            }
        }
        let model = instance(zero, Equality::EqualToZero);
        let state = State::from_iter([(0, 0.0), (1, 3.0)]);
        let solution = model.evaluate(&state, ATol::default()).unwrap();
        let mut proto: v2::Solution = solution.into();
        proto
            .evaluated_indicator_constraints
            .as_mut()
            .unwrap()
            .entries
            .get_mut(&7)
            .unwrap()
            .trigger_value = Some(u32::from(zero));
        assert!(Solution::from_v2_bytes(&proto.encode_to_vec()).is_err());
        let samples = model
            .evaluate_samples(&Sampled::from((0.into(), state)), ATol::default())
            .unwrap();
        let mut proto: v2::SampleSet = samples.into();
        proto
            .sampled_indicator_constraints
            .as_mut()
            .unwrap()
            .entries
            .get_mut(&7)
            .unwrap()
            .trigger_value = Some(u32::from(zero));
        assert!(SampleSet::from_v2_bytes(&proto.encode_to_vec()).is_err());
    }
}
