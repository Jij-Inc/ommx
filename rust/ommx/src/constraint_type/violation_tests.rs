use super::*;
use crate::{v1::State, IndicatorConstraint, OneHotConstraint, Sos1Constraint};
use proptest::prelude::*;

fn assert_contract<C: ConstraintType>(constraint: &C::Created, state: &State, atol: ATol) {
    let evaluated = constraint.evaluate(state, atol).unwrap();
    let violation = evaluated.violation();
    assert!(violation >= 0.0);
    assert_eq!(evaluated.is_feasible(), violation <= *atol);
    let samples = crate::Sampled::new([vec![0.into(), 1.into()]], [state.clone()]).unwrap();
    let sampled = constraint.evaluate_samples(&samples, atol).unwrap();
    for id in [0.into(), 1.into()] {
        assert_eq!(sampled.violation_for(id), Some(violation));
        assert_eq!(sampled.is_feasible_for(id), Some(evaluated.is_feasible()));
        let extracted = sampled.get(id).unwrap();
        assert_eq!(extracted.violation(), violation);
        assert_eq!(extracted.is_feasible(), evaluated.is_feasible());
        assert_eq!(extracted.feasibility_atol(), atol);
    }
    assert_eq!(sampled.violation_for(99.into()), None);
    assert_eq!(sampled.is_feasible_for(99.into()), None);
}

#[test]
fn every_family_uses_the_same_inclusive_scalar_threshold() {
    for tolerance in [0.125, 2.0_f64.powi(-20), 2.0_f64.powi(-40)] {
        let atol = ATol::new(tolerance).unwrap();
        for multiplier in [0.0, 0.5, 1.0, 1.25] {
            let value = tolerance * multiplier;
            let state = State::from_iter([(0, 1.0), (1, value), (2, 0.0)]);
            for equality in [
                crate::Equality::EqualToZero,
                crate::Equality::LessThanOrEqualToZero,
            ] {
                let function = crate::Function::from(crate::linear!(1));
                let constraint = Constraint {
                    equality,
                    stage: crate::CreatedData {
                        function: function.clone(),
                    },
                };
                assert_contract::<Constraint>(&constraint, &state, atol);
                let indicator = IndicatorConstraint::new(0.into(), equality, function);
                assert_contract::<IndicatorConstraint>(&indicator, &state, atol);
                let mut off = state.clone();
                off.entries.insert(0, 0.0);
                assert_contract::<IndicatorConstraint>(&indicator, &off, atol);
                assert_eq!(indicator.evaluate(&off, atol).unwrap().violation(), 0.0);
            }
            let variables = [0.into(), 1.into(), 2.into()].into_iter().collect();
            let one_hot = OneHotConstraint::new(variables).unwrap();
            let sos1 = Sos1Constraint::new(one_hot.variables.clone()).unwrap();
            assert_contract::<OneHotConstraint>(&one_hot, &state, atol);
            assert_contract::<Sos1Constraint>(&sos1, &state, atol);
            assert_eq!(one_hot.evaluate(&state, atol).unwrap().violation(), value);
            assert_eq!(sos1.evaluate(&state, atol).unwrap().violation(), value);
        }
    }
}

#[test]
fn small_member_errors_accumulate_without_input_canonicalization() {
    for tolerance in [2.0_f64.powi(-10), 2.0_f64.powi(-20), 2.0_f64.powi(-40)] {
        let atol = ATol::new(tolerance).unwrap();
        let variables = [0.into(), 1.into(), 2.into()].into_iter().collect();
        let one_hot = OneHotConstraint::new(variables).unwrap();
        let sos1 = Sos1Constraint::new(one_hot.variables.clone()).unwrap();
        let state = State::from_iter([(0, 1.0), (1, 0.75 * tolerance), (2, 0.75 * tolerance)]);
        let one_hot_result = one_hot.evaluate(&state, atol).unwrap();
        let sos1_result = sos1.evaluate(&state, atol).unwrap();
        assert_eq!(one_hot_result.violation(), 1.5 * tolerance);
        assert_eq!(sos1_result.violation(), 1.5 * tolerance);
        assert!(!one_hot_result.is_feasible());
        assert!(!sos1_result.is_feasible());
        assert_eq!(one_hot_result.stage.active_variable, None);
        assert_eq!(sos1_result.stage.active_variable, None);
        assert_contract::<OneHotConstraint>(&one_hot, &state, atol);
        assert_contract::<Sos1Constraint>(&sos1, &state, atol);
    }
}

proptest! {
    #[test]
    fn structural_metrics_match_minimum_changes_and_roundtrip(values in prop::array::uniform3(-4.0_f64..4.0)) {
        let atol = ATol::new(1e-4).unwrap();
        let state = State::from_iter(values.into_iter().enumerate().map(|(id, value)| (id as u64, value)));
        let variables = [0.into(), 1.into(), 2.into()].into_iter().collect();
        let one_hot = OneHotConstraint::new(variables).unwrap();
        let sos1 = Sos1Constraint::new(one_hot.variables.clone()).unwrap();
        let expected_one_hot = (0..3).map(|selected| values.iter().enumerate().map(|(i, x)| (x - if i == selected { 1.0 } else { 0.0 }).abs()).sum::<f64>()).fold(f64::INFINITY, f64::min);
        let expected_sos1 = (0..3).map(|selected| values.iter().enumerate().filter(|(i, _)| *i != selected).map(|(_, x)| x.abs()).sum::<f64>()).fold(f64::INFINITY, f64::min);
        let evaluated_one_hot = one_hot.evaluate(&state, atol).unwrap();
        let evaluated_sos1 = sos1.evaluate(&state, atol).unwrap();
        prop_assert!((evaluated_one_hot.violation() - expected_one_hot).abs() < 1e-12);
        prop_assert_eq!(evaluated_sos1.violation(), expected_sos1);
        assert_contract::<OneHotConstraint>(&one_hot, &state, atol);
        assert_contract::<Sos1Constraint>(&sos1, &state, atol);
        let wire: crate::v2::EvaluatedOneHotConstraint = evaluated_one_hot.clone().into();
        prop_assert_eq!(wire.parse_with_values(|id| state.entries.get(&id.into_inner()).copied(), atol).unwrap(), evaluated_one_hot.clone());
        let wire: crate::v2::EvaluatedSos1Constraint = evaluated_sos1.clone().into();
        prop_assert_eq!(wire.parse_with_values(|id| state.entries.get(&id.into_inner()).copied(), atol).unwrap(), evaluated_sos1.clone());
        let samples = crate::Sampled::from((SampleID::from(0), state));
        let wire: crate::v2::SampledOneHotConstraint = one_hot.evaluate_samples(&samples, atol).unwrap().into();
        prop_assert_eq!(wire.parse_with_values(|sid, id| samples.get(sid).and_then(|state| state.entries.get(&id.into_inner())).copied(), atol).unwrap().get(0.into()).unwrap(), evaluated_one_hot);
        let wire: crate::v2::SampledSos1Constraint = sos1.evaluate_samples(&samples, atol).unwrap().into();
        prop_assert_eq!(wire.parse_with_values(|sid, id| samples.get(sid).and_then(|state| state.entries.get(&id.into_inner())).copied(), atol).unwrap().get(0.into()).unwrap(), evaluated_sos1);
    }
}

#[test]
fn overflowed_violations_are_infeasible_and_survive_wire_roundtrip() {
    let atol = ATol::default();
    let variables = [0.into(), 1.into(), 2.into()].into_iter().collect();
    let constraint = Sos1Constraint::new(variables).unwrap();
    let state = State::from_iter([(0, f64::MAX), (1, f64::MAX), (2, f64::MAX)]);
    let evaluated = constraint.evaluate(&state, atol).unwrap();
    assert_eq!(evaluated.violation(), f64::INFINITY);
    assert!(!evaluated.is_feasible());
    let wire: crate::v2::EvaluatedSos1Constraint = evaluated.clone().into();
    assert_eq!(
        wire.parse_with_values(|id| state.entries.get(&id.into_inner()).copied(), atol)
            .unwrap(),
        evaluated
    );
}

#[test]
fn restoration_rejects_invalid_member_values_and_inconsistent_feasibility() {
    let atol = ATol::default();
    for value in [
        None,
        Some(f64::NAN),
        Some(f64::NEG_INFINITY),
        Some(f64::INFINITY),
    ] {
        let wire = crate::v2::EvaluatedSos1Constraint {
            variables: vec![0],
            ..Default::default()
        };
        assert!(wire.parse_with_values(|_| value, atol).is_err());
    }
    let wire = crate::v2::EvaluatedSos1Constraint {
        variables: vec![0],
        feasible: false,
        ..Default::default()
    };
    assert!(wire
        .parse_with_values(|_| Some(0.0), atol)
        .unwrap_err()
        .to_string()
        .contains("feasible must equal"));
}

#[test]
fn wire_consumers_can_read_feasibility_and_its_tolerance_without_sdk_evaluation() {
    use prost::Message;

    let instance = crate::Instance::builder()
        .sense(crate::Sense::Minimize)
        .objective(crate::Function::Zero)
        .decision_variables(
            (0..3)
                .map(|id| (id.into(), crate::DecisionVariable::continuous()))
                .collect(),
        )
        .constraints(BTreeMap::new())
        .sos1_constraints(BTreeMap::from([(
            0.into(),
            Sos1Constraint::new((0..3).map(Into::into).collect()).unwrap(),
        )]))
        .build()
        .unwrap();
    let state = State::from_iter([(0, 1.0), (1, 0.00006), (2, 0.00006)]);
    let samples = crate::Sampled::from((7.into(), state.clone()));

    for (tolerance, expected) in [(0.0001, false), (0.0002, true)] {
        let atol = ATol::new(tolerance).unwrap();
        let solution_bytes = instance.evaluate(&state, atol).unwrap().to_v2_bytes();
        // A consumer only needs the generated protobuf types to read these fields.
        let wire = crate::v2::Solution::decode(solution_bytes.as_slice()).unwrap();
        assert_eq!(wire.feasibility_atol, Some(tolerance));
        assert_eq!(wire.feasible, expected);
        assert_eq!(wire.feasible_relaxed, Some(expected));
        let row = &wire.evaluated_sos1_constraints.as_ref().unwrap().entries[&0];
        assert_eq!(row.feasible, expected);
        let restored = crate::Solution::from_v2_bytes(&solution_bytes).unwrap();
        assert_eq!(restored.sos1_constraint_violation(0.into()), Some(0.00012));
        assert_eq!(restored.feasible(), expected);

        let sample_bytes = instance
            .evaluate_samples(&samples, atol)
            .unwrap()
            .to_v2_bytes();
        let wire = crate::v2::SampleSet::decode(sample_bytes.as_slice()).unwrap();
        assert_eq!(wire.feasibility_atol, Some(tolerance));
        assert_eq!(wire.feasible[&7], expected);
        assert_eq!(wire.feasible_relaxed[&7], expected);
        let row = &wire.sampled_sos1_constraints.as_ref().unwrap().entries[&0];
        assert_eq!(row.feasible[&7], expected);
        let restored = crate::SampleSet::from_v2_bytes(&sample_bytes)
            .unwrap()
            .get(7.into())
            .unwrap();
        assert_eq!(restored.sos1_constraint_violation(0.into()), Some(0.00012));
        assert_eq!(restored.feasible(), expected);
    }
}

#[test]
fn partial_evaluation_preserves_instance_invariants_and_failure_is_atomic() {
    let atol = ATol::new(0.125).unwrap();
    let mut instance = crate::Instance::builder()
        .sense(crate::Sense::Minimize)
        .objective(crate::Function::Zero)
        .decision_variables(
            (0..3)
                .map(|id| (id.into(), crate::DecisionVariable::continuous()))
                .collect(),
        )
        .constraints(BTreeMap::new())
        .sos1_constraints(BTreeMap::from([(
            0.into(),
            Sos1Constraint::new((0..3).map(Into::into).collect()).unwrap(),
        )]))
        .build()
        .unwrap();
    let original = instance.clone();
    let fixed = State::from_iter([(1, 0.09375), (2, 0.09375)]);
    // Dropping the fixed near-zero members would leave a singleton SOS1 and
    // incorrectly make this infeasible completion feasible.
    assert!(!original
        .evaluate(
            &State::from_iter([(0, 1.0), (1, 0.09375), (2, 0.09375)]),
            atol
        )
        .unwrap()
        .feasible());
    let error = instance.partial_evaluate(&fixed, atol).unwrap_err();
    assert!(error
        .to_string()
        .contains("without changing constraint feasibility"));
    assert_eq!(instance, original);
    assert!(original
        .clone()
        .into_partial_evaluated(&fixed, atol)
        .is_err());

    let zero = State::from_iter([(2, 0.0)]);
    instance.partial_evaluate(&zero, atol).unwrap();
    let restored = crate::Instance::from_v2_bytes(&instance.to_v2_bytes()).unwrap();
    let state = State::from_iter([(0, 1.0), (1, 0.09375)]);
    let result = restored.evaluate(&state, atol).unwrap();
    assert_eq!(result.total_violation(), 0.09375);
    assert!(result.feasible());
}
