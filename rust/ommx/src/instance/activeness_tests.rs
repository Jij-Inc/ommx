use super::*;
use crate::{coeff, linear, ATol, ConstraintActiveness as A, Equality, Sampled, Sense};

fn metadata(skip: bool) -> InactiveConstraintMetadata {
    InactiveConstraintMetadata {
        reason: "test.condition".into(),
        parameters: [("condition".into(), "false".into())].into_iter().collect(),
        skip_feasibility_check: skip,
    }
}

fn all_families() -> Instance {
    let mut instance = Instance::builder()
        .sense(Sense::Minimize)
        .objective(Function::Zero)
        .decision_variables(BTreeMap::from([
            (0.into(), DecisionVariable::binary()),
            (1.into(), DecisionVariable::binary()),
        ]))
        .active_constraints(
            (0..3)
                .map(|i| {
                    (
                        i.into(),
                        Constraint::less_than_or_equal_to_zero(Function::from(coeff!(1.0))),
                    )
                })
                .collect(),
        )
        .active_indicator_constraints(
            (3..6)
                .map(|i| {
                    (
                        i.into(),
                        IndicatorConstraint::new(
                            0.into(),
                            Equality::LessThanOrEqualToZero,
                            Function::from(coeff!(1.0)),
                        ),
                    )
                })
                .collect(),
        )
        .active_one_hot_constraints(
            (6..9)
                .map(|i| {
                    (
                        i.into(),
                        OneHotConstraint::new([0.into(), 1.into()].into()).unwrap(),
                    )
                })
                .collect(),
        )
        .active_sos1_constraints(
            (9..12)
                .map(|i| {
                    (
                        i.into(),
                        Sos1Constraint::new([0.into(), 1.into()].into()).unwrap(),
                    )
                })
                .collect(),
        )
        .build()
        .unwrap();
    for (offset, skip) in [(1, false), (2, true)] {
        instance
            .deactivate_constraint(offset.into(), "test".into(), [], skip)
            .unwrap();
        instance
            .deactivate_indicator_constraint((3 + offset).into(), "test".into(), [], skip)
            .unwrap();
        instance
            .deactivate_one_hot_constraint((6 + offset).into(), "test".into(), [], skip)
            .unwrap();
        instance
            .deactivate_sos1_constraint((9 + offset).into(), "test".into(), [], skip)
            .unwrap();
    }
    instance
}

#[test]
fn ordered_selection_counts_the_same_rows_across_families() {
    assert!(A::InactiveSkipped < A::InactiveChecked && A::InactiveChecked < A::Active);
    assert_eq!(A::default(), A::InactiveChecked);
    assert_eq!(
        InactiveConstraintMetadata::default().activeness(),
        A::InactiveChecked
    );
    assert_eq!(metadata(false).activeness(), A::InactiveChecked);
    assert_eq!(metadata(true).activeness(), A::InactiveSkipped);
    let instance = all_families();
    assert_eq!(instance.constraints().len(), 3);
    assert!(instance.constraints().get(&2.into()).is_some());
    assert_eq!(instance.active_constraints().len(), 1);
    assert_eq!(instance.inactive_constraints().len(), 2);
    let state = v1::State::from_iter([(0, 1.0), (1, 1.0)]);
    let solution = instance.evaluate(&state, ATol::default()).unwrap();
    let samples = instance
        .evaluate_samples(&Sampled::from((0.into(), state)), ATol::default())
        .unwrap();
    for (minimum, offsets) in [
        (A::Active, &[0_u64][..]),
        (A::InactiveChecked, &[0, 1][..]),
        (A::InactiveSkipped, &[0, 1, 2][..]),
    ] {
        let count = offsets.len();
        assert_eq!(
            solution
                .evaluated_constraints()
                .selected(minimum)
                .map(|(id, _)| id.into_inner())
                .collect::<Vec<_>>(),
            offsets,
        );
        assert_eq!(
            solution
                .evaluated_indicator_constraints()
                .selected(minimum)
                .map(|(id, _)| id.into_inner())
                .collect::<Vec<_>>(),
            offsets.iter().map(|id| id + 3).collect::<Vec<_>>(),
        );
        assert_eq!(
            solution
                .evaluated_one_hot_constraints()
                .selected(minimum)
                .map(|(id, _)| id.into_inner())
                .collect::<Vec<_>>(),
            offsets.iter().map(|id| id + 6).collect::<Vec<_>>(),
        );
        assert_eq!(
            solution
                .evaluated_sos1_constraints()
                .selected(minimum)
                .map(|(id, _)| id.into_inner())
                .collect::<Vec<_>>(),
            offsets.iter().map(|id| id + 9).collect::<Vec<_>>(),
        );
        assert_eq!(solution.total_violation(minimum), 4.0 * count as f64);
        assert_eq!(samples.constraints().selected(minimum).count(), count);
        assert_eq!(
            samples.indicator_constraints().selected(minimum).count(),
            count
        );
        assert_eq!(
            samples.one_hot_constraints().selected(minimum).count(),
            count
        );
        assert_eq!(samples.sos1_constraints().selected(minimum).count(), count);
        assert_eq!(
            samples.get(0.into()).unwrap().total_violation(minimum),
            solution.total_violation(minimum)
        );
    }
}

#[test]
fn sample_selection_preserves_feasible_and_relaxed_judgments() {
    let x = Function::from(linear!(0));
    let instance = Instance::builder()
        .sense(Sense::Minimize)
        .objective(Function::from(-linear!(0)))
        .decision_variables([(0.into(), DecisionVariable::continuous())].into())
        .active_constraints(BTreeMap::new())
        .inactive_constraints(
            [
                (
                    0.into(),
                    (
                        Constraint::less_than_or_equal_to_zero((x.clone() - coeff!(1.0)).unwrap()),
                        metadata(false),
                    ),
                ),
                (
                    1.into(),
                    (Constraint::less_than_or_equal_to_zero(x), metadata(true)),
                ),
            ]
            .into(),
        )
        .build()
        .unwrap();
    let states = Sampled::new(
        [vec![0.into()], vec![1.into()], vec![2.into()]],
        [2.0, 1.0, 0.0].map(|x| v1::State::from_iter([(0, x)])),
    )
    .unwrap();
    let samples = instance.evaluate_samples(&states, ATol::default()).unwrap();
    assert_eq!(samples.best_feasible_id().unwrap(), 1.into());
    assert_eq!(samples.best_feasible_relaxed_id().unwrap(), 0.into());
    assert_eq!(samples.feasible_unrelaxed_ids(), samples.feasible_ids());
    for (id, expected) in [false, true, true].into_iter().enumerate() {
        let id = (id as u64).into();
        assert_eq!(samples.feasible()[&id], expected);
        assert_eq!(samples.feasible_ids().contains(&id), expected);
        assert_eq!(samples.is_sample_feasible(id), Some(expected));
        let solution = samples.get(id).unwrap();
        assert_eq!(solution.feasible(), expected);
        assert_eq!(solution.feasible_constraints(), expected);
        assert!(samples.feasible_relaxed()[&id]);
        assert!(samples.feasible_relaxed_ids().contains(&id));
        assert_eq!(samples.is_sample_feasible_relaxed(id), Some(true));
        assert!(solution.feasible_relaxed());
    }
    assert_eq!(samples.is_sample_feasible(99.into()), None);
    assert_eq!(samples.is_sample_feasible_relaxed(99.into()), None);
    let best = samples.best_feasible().unwrap();
    assert!(best.feasible());
    assert_eq!(best.total_violation(A::InactiveChecked), 0.0);
    assert_eq!(best.total_violation(A::InactiveSkipped), 1.0);
    let best_relaxed = samples.best_feasible_relaxed().unwrap();
    assert!(best_relaxed.feasible_relaxed());
    assert!(!best_relaxed.feasible());
}

#[test]
fn structural_activation_failure_is_atomic_and_preserves_metadata() {
    let mut instance = Instance::builder()
        .sense(Sense::Minimize)
        .objective(Function::Zero)
        .decision_variables(
            [
                (0.into(), DecisionVariable::binary()),
                (1.into(), DecisionVariable::binary()),
            ]
            .into(),
        )
        .active_constraints(BTreeMap::new())
        .inactive_one_hot_constraints(
            [(
                10.into(),
                (
                    OneHotConstraint::new([0.into(), 1.into()].into()).unwrap(),
                    metadata(true),
                ),
            )]
            .into(),
        )
        .inactive_sos1_constraints(
            [(
                11.into(),
                (
                    Sos1Constraint::new([0.into(), 1.into()].into()).unwrap(),
                    metadata(false),
                ),
            )]
            .into(),
        )
        .build()
        .unwrap();
    instance
        .partial_evaluate(&v1::State::from_iter([(0, 1.0)]), ATol::default())
        .unwrap();
    let before = instance.clone();
    assert!(instance.activate_one_hot_constraint(10.into()).is_err());
    assert!(instance.activate_sos1_constraint(11.into()).is_err());
    assert_eq!(
        instance.inactive_one_hot_constraints(),
        before.inactive_one_hot_constraints()
    );
    assert_eq!(
        instance.inactive_sos1_constraints(),
        before.inactive_sos1_constraints()
    );
    assert!(instance.active_one_hot_constraints().is_empty());
    assert!(instance.active_sos1_constraints().is_empty());
    instance
        .set_one_hot_constraint_skip_feasibility_check(10.into(), false)
        .unwrap();
    assert_eq!(
        instance.inactive_one_hot_constraints()[&10.into()].1,
        metadata(false)
    );
}

#[test]
fn parametric_transitions_preserve_ids_context_and_inactive_policy() {
    let mut instance = ParametricInstance::from(all_families());
    let context = ConstraintContext {
        label: ModelingLabel {
            name: Some("condition".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    instance
        .set_constraint_context(2.into(), context.clone())
        .unwrap();
    assert!(instance
        .deactivate_constraint(2.into(), "duplicate".into(), [], false)
        .is_err());
    assert!(instance
        .set_constraint_skip_feasibility_check(0.into(), true)
        .is_err());
    instance.activate_constraint(2.into()).unwrap();
    instance.activate_indicator_constraint(5.into()).unwrap();
    instance.activate_one_hot_constraint(8.into()).unwrap();
    instance.activate_sos1_constraint(11.into()).unwrap();
    assert_eq!(instance.constraints().len(), 3);
    assert_eq!(instance.active_constraints().len(), 2);
    assert_eq!(instance.constraint_context().collect_for(2.into()), context);
    instance
        .deactivate_constraint(2.into(), "again".into(), [], true)
        .unwrap();
    assert_eq!(
        instance.constraints().activeness(&2.into()),
        Some(A::InactiveSkipped)
    );
    assert_eq!(instance.constraint_context().collect_for(2.into()), context);
}

#[test]
fn skipped_constraints_still_propagate_numeric_evaluation_errors() {
    let function = crate::function::operation::unary_expression(
        crate::function::operation::UnaryOperator::Powi(-1),
        Function::from(linear!(0)),
    );
    let instance = Instance::builder()
        .sense(Sense::Minimize)
        .objective(Function::Zero)
        .decision_variables([(0.into(), DecisionVariable::continuous())].into())
        .active_constraints(BTreeMap::new())
        .inactive_constraints(
            [(
                0.into(),
                (
                    Constraint::less_than_or_equal_to_zero(function),
                    metadata(true),
                ),
            )]
            .into(),
        )
        .build()
        .unwrap();
    let state = v1::State::from_iter([(0, 0.0)]);
    assert!(instance.evaluate(&state, ATol::default()).is_err());
    assert!(instance
        .evaluate_samples(&Sampled::from((0.into(), state)), ATol::default())
        .is_err());
}

#[test]
fn specialization_and_partial_evaluation_preserve_inactive_policy() {
    let function = Function::from(linear!(0) + linear!(100));
    let parametric = ParametricInstance::builder()
        .sense(Sense::Minimize)
        .objective(Function::Zero)
        .decision_variables([(0.into(), DecisionVariable::binary())].into())
        .parameters(ParameterTable::from_ids([100.into()].into()))
        .active_constraints(BTreeMap::new())
        .inactive_constraints(
            [(
                0.into(),
                (
                    Constraint::less_than_or_equal_to_zero(function.clone()),
                    metadata(true),
                ),
            )]
            .into(),
        )
        .inactive_indicator_constraints(
            [(
                1.into(),
                (
                    IndicatorConstraint::new(0.into(), Equality::LessThanOrEqualToZero, function),
                    metadata(true),
                ),
            )]
            .into(),
        )
        .inactive_one_hot_constraints(
            [(
                2.into(),
                (
                    OneHotConstraint::new([0.into()].into()).unwrap(),
                    metadata(true),
                ),
            )]
            .into(),
        )
        .inactive_sos1_constraints(
            [(
                3.into(),
                (
                    Sos1Constraint::new([0.into()].into()).unwrap(),
                    metadata(true),
                ),
            )]
            .into(),
        )
        .build()
        .unwrap();
    let mut instance = parametric
        .with_parameters(v1::Parameters {
            entries: [(100, 2.0)].into(),
        })
        .unwrap();
    instance
        .partial_evaluate(&v1::State::from_iter([(0, 1.0)]), ATol::default())
        .unwrap();
    assert_eq!(instance.inactive_constraints()[&0.into()].1, metadata(true));
    assert_eq!(
        instance.inactive_indicator_constraints()[&1.into()].1,
        metadata(true)
    );
    assert_eq!(
        instance.inactive_one_hot_constraints()[&2.into()].1,
        metadata(true)
    );
    assert_eq!(
        instance.inactive_sos1_constraints()[&3.into()].1,
        metadata(true)
    );
    let solution = instance
        .evaluate(&v1::State::default(), ATol::default())
        .unwrap();
    assert!(solution.feasible());
    assert!(solution.feasible_relaxed());
    assert_eq!(solution.total_violation(A::InactiveChecked), 0.0);
    assert!(solution.total_violation(A::InactiveSkipped) > 0.0);
    assert_eq!(solution.constraint_violation(0.into()), Some(3.0));
    assert_eq!(solution.indicator_constraint_violation(1.into()), Some(3.0));
}
