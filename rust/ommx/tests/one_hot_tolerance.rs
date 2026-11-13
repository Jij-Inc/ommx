use ommx::{
    v1::State, v2, ATol, DecisionVariable, Evaluate, Function, Instance, OneHotConstraint,
    SampleSet, Sampled, SampledDecisionVariable, Sense, Solution, Sos1Constraint,
};
use std::collections::BTreeMap;

fn model() -> Instance {
    Instance::builder()
        .sense(Sense::Minimize)
        .objective(Function::Zero)
        .decision_variables(
            (0..3)
                .map(|id| (id.into(), DecisionVariable::binary()))
                .collect(),
        )
        .active_constraints(BTreeMap::new())
        .active_one_hot_constraints(
            [(
                0.into(),
                OneHotConstraint::new([0.into(), 1.into(), 2.into()].into()).unwrap(),
            )]
            .into(),
        )
        .build()
        .unwrap()
}

#[test]
fn instance_evaluation_and_partial_evaluation_reject_large_one_hot_tolerances() {
    let instance = model();
    let state = State::from_iter([(0, 1.0), (1, 0.0), (2, 0.0)]);
    for value in [0.5, 1.0, f64::INFINITY] {
        let atol = ATol::new(value).unwrap();
        assert!(instance
            .evaluate(&state, atol)
            .unwrap_err()
            .to_string()
            .contains("require atol < 0.5"));
        for states in [Sampled::default(), Sampled::from((7.into(), state.clone()))] {
            assert!(instance
                .evaluate_samples(&states, atol)
                .unwrap_err()
                .to_string()
                .contains("require atol < 0.5"));
        }
        let mut partial = instance.clone();
        assert!(partial
            .partial_evaluate(&state, atol)
            .unwrap_err()
            .to_string()
            .contains("require atol < 0.5"));
        assert_eq!(partial, instance);
    }
    let atol = ATol::new(0.5_f64.next_down()).unwrap();
    assert!(instance.evaluate(&state, atol).unwrap().feasible());
    let mut partial = instance;
    partial.partial_evaluate(&state, atol).unwrap();
    assert!(partial
        .evaluate(&State::default(), atol)
        .unwrap()
        .feasible());
}

#[test]
fn result_restoration_rejects_large_one_hot_tolerances_even_without_samples() {
    let instance = model();
    let state = State::from_iter([(0, 1.0), (1, 0.0), (2, 0.0)]);
    let solution = instance.evaluate(&state, ATol::default()).unwrap();
    let mut wire: v2::Solution = solution.into();
    wire.feasibility_atol = Some(0.5);
    let error = Solution::try_from(wire).unwrap_err();
    assert!(error.to_string().contains("require atol < 0.5"), "{error}");

    for states in [Sampled::default(), Sampled::from((7.into(), state))] {
        let samples = instance.evaluate_samples(&states, ATol::default()).unwrap();
        let mut wire: v2::SampleSet = samples.into();
        wire.feasibility_atol = Some(0.5);
        let error = SampleSet::try_from(wire).unwrap_err();
        assert!(error.to_string().contains("require atol < 0.5"), "{error}");
    }
}

#[test]
fn sample_builder_cannot_bypass_validation_with_an_empty_sampled_row() {
    let mut row = OneHotConstraint::new([0.into()].into())
        .unwrap()
        .evaluate_samples(&Sampled::default(), ATol::default())
        .unwrap();
    row.stage.activation_atol = ATol::new(0.5).unwrap();
    let error = SampleSet::builder()
        .sense(Sense::Minimize)
        .feasibility_atol(row.stage.activation_atol)
        .objectives(Sampled::default())
        .constraints(BTreeMap::new())
        .one_hot_constraints([(0.into(), row)].into())
        .decision_variables(
            [(
                0.into(),
                SampledDecisionVariable::new(
                    0.into(),
                    DecisionVariable::binary(),
                    Sampled::default(),
                )
                .unwrap(),
            )]
            .into(),
        )
        .build()
        .unwrap_err();
    assert!(error.to_string().contains("require atol < 0.5"), "{error}");
}

#[test]
fn large_general_tolerances_remain_available_without_one_hot() {
    let atol = ATol::new(1.0).unwrap();
    let instance = Instance::builder()
        .sense(Sense::Minimize)
        .objective(Function::Zero)
        .decision_variables(BTreeMap::new())
        .active_constraints(BTreeMap::new())
        .build()
        .unwrap();
    assert!(instance.evaluate(&State::default(), atol).is_ok());
    assert!(Sos1Constraint::new([0.into(), 1.into()].into())
        .unwrap()
        .evaluate(&State::from_iter([(0, 1.0), (1, 1.0)]), atol)
        .is_ok());
}
