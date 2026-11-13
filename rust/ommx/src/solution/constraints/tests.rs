use super::*;
use crate::v1::State;
use crate::{
    ATol, Evaluate, EvaluatedConstraintBehavior, OneHotConstraint, OneHotViolationDiagnostics,
    Sos1Constraint, VariableIDSet,
};
use crate::{SampleID, SampleSet};
use proptest::prelude::*;

fn instance(n: usize) -> crate::Instance {
    let ids: VariableIDSet = (0..n as u64).map(Into::into).collect();
    crate::Instance::builder()
        .sense(crate::Sense::Minimize)
        .objective(crate::Function::Zero)
        .active_constraints(BTreeMap::new())
        .decision_variables(
            ids.iter()
                .map(|&id| (id, crate::DecisionVariable::binary()))
                .collect(),
        )
        .active_one_hot_constraints(
            [(0.into(), crate::OneHotConstraint::new(ids.clone()).unwrap())].into(),
        )
        .active_sos1_constraints([(1.into(), crate::Sos1Constraint::new(ids).unwrap())].into())
        .build()
        .unwrap()
}

fn evaluate(values: [f64; 3], atol: f64) -> Solution {
    instance(3)
        .evaluate(
            &crate::v1::State::from_iter(
                values
                    .into_iter()
                    .enumerate()
                    .map(|(id, value)| (id as u64, value)),
            ),
            ATol::new(atol).unwrap(),
        )
        .unwrap()
}

#[test]
fn missing_selection_uses_canonical_stored_values() {
    for values in [[0.0, 0.0, -0.0], [0.0625, -0.0625, 0.0]] {
        let result = evaluate(values, 0.125);
        let Some(OneHotViolationDiagnostics::MissingSelection { values: retained }) = result
            .violation_diagnostics::<OneHotConstraint>(ConstraintRole::EvaluationOnly)
            .remove(&0.into())
        else {
            panic!("expected a missing selection");
        };
        // Instance evaluation canonicalizes binary values within tolerance.
        assert_eq!(
            retained,
            [(0.into(), 0.0), (1.into(), 0.0), (2.into(), 0.0)].into()
        );
        for (&id, &value) in &retained {
            assert_eq!(value, *result.decision_variables()[&id].value());
        }
    }
}

#[test]
fn multiple_selections_preserve_other_value_errors() {
    let result = evaluate([1.0, 0.9375, -0.5], 0.125);
    assert_eq!(result.one_hot_constraint_violation(0.into()).unwrap(), 1.0);
    assert_eq!(
        result
            .violation_diagnostics::<OneHotConstraint>(ConstraintRole::EvaluationOnly)
            .remove(&0.into()),
        Some(OneHotViolationDiagnostics::MultipleSelections {
            selected_variables: [0.into(), 1.into()].into(),
            values: [(0.into(), 1.0), (1.into(), 1.0), (2.into(), -0.5)].into(),
        })
    );
}

#[test]
fn value_errors_report_absolute_changes_and_a_deterministic_target() {
    let result = evaluate([0.875, 0.25, 0.0], 0.0625);
    assert_eq!(result.one_hot_constraint_violation(0.into()).unwrap(), 0.25);
    assert_eq!(
        result
            .violation_diagnostics::<OneHotConstraint>(ConstraintRole::EvaluationOnly)
            .remove(&0.into()),
        Some(OneHotViolationDiagnostics::ValueErrors {
            target_variable: 0.into(),
            errors: [(0.into(), 0.125), (1.into(), 0.25)].into(),
            values: [(0.into(), 0.875), (1.into(), 0.25), (2.into(), 0.0)].into(),
        })
    );
    let Some(OneHotViolationDiagnostics::ValueErrors {
        target_variable,
        errors,
        ..
    }) = evaluate([0.5, 0.5, 0.0], 0.125)
        .violation_diagnostics::<OneHotConstraint>(ConstraintRole::EvaluationOnly)
        .remove(&0.into())
    else {
        panic!()
    };
    assert_eq!(target_variable, 1.into());
    assert_eq!(errors, [(0.into(), 0.5), (1.into(), 0.5)].into());
    // Select by value, not absolute value; negative outliers remain errors.
    let Some(OneHotViolationDiagnostics::ValueErrors {
        target_variable,
        errors,
        ..
    }) = evaluate([-3.0, 0.875, 0.0], 0.0625)
        .violation_diagnostics::<OneHotConstraint>(ConstraintRole::EvaluationOnly)
        .remove(&0.into())
    else {
        panic!()
    };
    assert_eq!(target_variable, 1.into());
    assert_eq!(errors, [(0.into(), 3.0), (1.into(), 0.125)].into());
}

#[test]
fn inclusive_tolerance_and_overlapping_neighborhoods_follow_feasibility() {
    assert_eq!(
        evaluate([0.875, 0.125, 0.0], 0.125)
            .violation_diagnostics::<OneHotConstraint>(ConstraintRole::EvaluationOnly)
            .remove(&0.into()),
        None
    );
    assert_eq!(
        evaluate([0.0, 0.0, 0.0], 1.0)
            .violation_diagnostics::<OneHotConstraint>(ConstraintRole::EvaluationOnly)
            .remove(&0.into()),
        None
    );
    assert_eq!(
        evaluate([1.0, 1.0, 0.0], 1.0)
            .violation_diagnostics::<OneHotConstraint>(ConstraintRole::EvaluationOnly)
            .remove(&0.into()),
        None
    );
    let result = evaluate([0.875, 0.25, 0.0], 0.125);
    let before = result
        .violation_diagnostics::<OneHotConstraint>(ConstraintRole::EvaluationOnly)
        .remove(&0.into());
    assert!(result.evaluated_one_hot_constraints()[&0.into()].is_feasible(ATol::new(0.25).unwrap()));
    assert_eq!(
        result
            .violation_diagnostics::<OneHotConstraint>(ConstraintRole::EvaluationOnly)
            .remove(&0.into()),
        before
    );
    let Some(OneHotViolationDiagnostics::ValueErrors { errors, .. }) = before else {
        panic!()
    };
    assert_eq!(errors, [(1.into(), 0.25)].into());
}
#[test]
fn regular_and_indicator_violations_have_dedicated_diagnostics() {
    use crate::{
        Constraint, CreatedData, Equality, Function, IndicatorConstraint, IndicatorTriggerValue,
    };
    let regular = Constraint {
        equality: Equality::EqualToZero,
        stage: CreatedData {
            function: crate::linear!(0).into(),
        },
    };
    let indicator = IndicatorConstraint::new(
        0.into(),
        Equality::EqualToZero,
        crate::linear!(1).into(),
        IndicatorTriggerValue::One,
    );
    let host = crate::Instance::builder()
        .sense(crate::Sense::Minimize)
        .objective(Function::Zero)
        .decision_variables(
            [
                (0.into(), crate::DecisionVariable::binary()),
                (1.into(), crate::DecisionVariable::binary()),
            ]
            .into(),
        )
        .active_constraints([(10.into(), regular.clone())].into())
        .active_indicator_constraints([(11.into(), indicator.clone())].into())
        .build()
        .unwrap();
    let state = State::from_iter([(0, 1.0), (1, 1.0)]);
    let atol = ATol::default();
    let solution = host.evaluate(&state, atol).unwrap();
    let samples = host
        .evaluate_samples(&crate::Sampled::from((7.into(), state.clone())), atol)
        .unwrap();
    assert_eq!(
        solution.violation_diagnostics::<Constraint>(ConstraintRole::EvaluationOnly),
        [(10.into(), crate::RegularViolationDiagnostics {})].into()
    );
    assert_eq!(
        solution.violation_diagnostics::<IndicatorConstraint>(ConstraintRole::EvaluationOnly),
        [(11.into(), crate::IndicatorViolationDiagnostics {})].into()
    );
    assert_eq!(
        samples
            .violation_diagnostics::<Constraint>(7.into(), ConstraintRole::EvaluationOnly)
            .unwrap(),
        [(10.into(), crate::RegularViolationDiagnostics {})].into()
    );
    assert_eq!(
        samples
            .violation_diagnostics::<IndicatorConstraint>(7.into(), ConstraintRole::EvaluationOnly)
            .unwrap(),
        [(11.into(), crate::IndicatorViolationDiagnostics {})].into()
    );
    // Empty family queries still reject an unknown sample.
    let err = samples
        .violation_diagnostics::<OneHotConstraint>(99.into(), ConstraintRole::EvaluationOnly)
        .unwrap_err();
    assert!(matches!(
        err.downcast_ref::<crate::SampleSetError>(),
        Some(crate::SampleSetError::UnknownSampleID { .. })
    ));
    // Empty family diagnostics need no variable lookup, even internally.
    regular.evaluate(&state, atol).unwrap().diagnose(
        |_| panic!("regular diagnostics must not read variable values"),
        atol,
    );
    indicator.evaluate(&state, atol).unwrap().diagnose(
        |_| panic!("indicator diagnostics must not read variable values"),
        atol,
    );
    let feasible = host
        .evaluate(&State::from_iter([(0, 0.0), (1, 0.0)]), atol)
        .unwrap();
    assert!(feasible
        .violation_diagnostics::<Constraint>(ConstraintRole::EvaluationOnly)
        .is_empty());
    assert!(feasible
        .violation_diagnostics::<IndicatorConstraint>(ConstraintRole::EvaluationOnly)
        .is_empty());
}

#[test]
fn sampled_diagnostics_select_values_by_sample_id() {
    let host = instance(3);
    let atol = ATol::new(0.0625).unwrap();
    let states = crate::Sampled::new(
        [vec![2.into(), 7.into()], vec![13.into()]],
        [
            State::from_iter([(0, 0.875), (1, 0.25), (2, 0.0)]),
            State::from_iter([(0, 0.25), (1, 0.875), (2, 0.0)]),
        ],
    )
    .unwrap();
    let samples = host.evaluate_samples(&states, atol).unwrap();
    for (sample_id, target, excess) in [(13, 1, 0), (2, 0, 1), (7, 0, 1)] {
        let sample_id = SampleID::from(sample_id);
        let one_hot = samples
            .violation_diagnostics::<OneHotConstraint>(sample_id, ConstraintRole::EvaluationOnly)
            .unwrap()
            .remove(&0.into())
            .unwrap();
        let OneHotViolationDiagnostics::ValueErrors {
            target_variable,
            ref errors,
            ..
        } = one_hot
        else {
            panic!("expected value errors");
        };
        assert_eq!(target_variable, target.into());
        assert_eq!(
            *errors,
            [(target.into(), 0.125), (excess.into(), 0.25)].into()
        );
        let sos1 = samples
            .violation_diagnostics::<Sos1Constraint>(sample_id, ConstraintRole::EvaluationOnly)
            .unwrap()
            .remove(&1.into())
            .unwrap();
        assert_eq!(sos1.values, [(excess.into(), 0.25)].into());
        let solution = samples.get(sample_id).unwrap();
        assert_eq!(
            Solution::violation_diagnostics::<OneHotConstraint>(
                &solution,
                ConstraintRole::EvaluationOnly
            ),
            [(0.into(), one_hot)].into(),
        );
        assert_eq!(
            Solution::violation_diagnostics::<Sos1Constraint>(
                &solution,
                ConstraintRole::EvaluationOnly
            ),
            [(1.into(), sos1)].into(),
        );
    }
}

proptest::proptest! {
    #[test]
    fn diagnostics_match_feasibility_at_any_retained_tolerance(
        values in prop::collection::vec(-16_i16..=16, 1..8),
        tolerance in 1_u16..=32,
    ) {
        let atol = ATol::new(f64::from(tolerance) / 8.0).unwrap();
        let host = instance(values.len());
        let state = State::from_iter(values.into_iter().enumerate().map(|(id, value)| (id as u64, f64::from(value) / 8.0)));
        let solution = host.evaluate(&state, atol).unwrap();
        let one_hot = &solution.evaluated_one_hot_constraints()[&0.into()];
        let sos1 = &solution.evaluated_sos1_constraints()[&1.into()];
        let one_hot_details = solution.violation_diagnostics::<OneHotConstraint>(ConstraintRole::EvaluationOnly).remove(&0.into());
        let sos1_details = solution.violation_diagnostics::<Sos1Constraint>(ConstraintRole::EvaluationOnly);
        let samples = host.evaluate_samples(&crate::Sampled::new([vec![2.into(), 7.into()]], [state.clone()]).unwrap(), atol).unwrap();
        let restored = SampleSet::from_v2_bytes(&samples.to_v2_bytes()).unwrap();
        for samples in [&samples, &restored] {
            for sid in [2.into(), 7.into()] {
                prop_assert_eq!(samples.violation_diagnostics::<OneHotConstraint>(sid, ConstraintRole::EvaluationOnly).unwrap().remove(&0.into()), one_hot_details.clone());
                prop_assert_eq!(samples.violation_diagnostics::<Sos1Constraint>(sid, ConstraintRole::EvaluationOnly).unwrap(), sos1_details.clone());
                let extracted = samples.get(sid).unwrap();
                prop_assert_eq!(extracted.violation_diagnostics::<OneHotConstraint>(ConstraintRole::EvaluationOnly).remove(&0.into()), one_hot_details.clone());
                prop_assert_eq!(extracted.violation_diagnostics::<Sos1Constraint>(ConstraintRole::EvaluationOnly), sos1_details.clone());
            }
        }
        let restored = Solution::from_v2_bytes(&solution.to_v2_bytes()).unwrap();
        prop_assert_eq!(restored.violation_diagnostics::<OneHotConstraint>(ConstraintRole::EvaluationOnly).remove(&0.into()), one_hot_details.clone());
        prop_assert_eq!(restored.violation_diagnostics::<Sos1Constraint>(ConstraintRole::EvaluationOnly), sos1_details.clone());
        prop_assert_eq!(one_hot_details.is_none(), one_hot.is_feasible(atol));
        prop_assert_eq!(sos1_details.is_empty(), sos1.is_feasible(atol));
        for (id, value) in sos1_details.into_values().flat_map(|diagnostic| diagnostic.values) {
            prop_assert_eq!(*solution.decision_variables()[&id].value(), value);
            prop_assert!(value.abs() > *atol);
        }
        if let Some(crate::OneHotViolationDiagnostics::ValueErrors { errors, target_variable, values }) = one_hot_details {
            prop_assert!(!errors.is_empty());
            prop_assert_eq!(errors.values().copied().fold(0.0_f64, f64::max), one_hot.violation());
            for (id, error) in errors {
                prop_assert!(error > *atol);
                prop_assert_eq!(error, (values[&id] - if id == target_variable { 1.0 } else { 0.0 }).abs());
            }
        }
    }
}
