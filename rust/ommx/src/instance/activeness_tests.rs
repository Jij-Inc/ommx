use super::*;
use crate::{coeff, linear, ATol, Equality, Sense};

fn metadata() -> InactiveConstraintMetadata {
    InactiveConstraintMetadata {
        reason: "test.condition".into(),
        parameters: [("condition".into(), "false".into())].into_iter().collect(),
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
    for offset in [1, 2] {
        instance
            .deactivate_constraint(offset.into(), "test".into(), [])
            .unwrap();
        instance
            .deactivate_indicator_constraint((3 + offset).into(), "test".into(), [])
            .unwrap();
        instance
            .deactivate_one_hot_constraint((6 + offset).into(), "test".into(), [])
            .unwrap();
        instance
            .deactivate_sos1_constraint((9 + offset).into(), "test".into(), [])
            .unwrap();
    }
    instance
}

#[test]
fn all_state_lookups_include_inactive_rows_in_each_family() {
    let instance = all_families();
    assert_eq!(instance.constraints().len(), 3);
    assert!(instance.constraints().get(&2.into()).is_some());
    assert_eq!(instance.active_constraints().len(), 1);
    assert_eq!(instance.inactive_constraints().len(), 2);
    assert_eq!(instance.indicator_constraints().len(), 3);
    assert_eq!(instance.one_hot_constraints().len(), 3);
    assert_eq!(instance.sos1_constraints().len(), 3);
    let state = v1::State::from_iter([(0, 1.0), (1, 1.0)]);
    let solution = instance.evaluate(&state, ATol::default()).unwrap();
    assert_eq!(solution.total_violation(), 12.0);
    assert!(!solution.feasible());
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
                    metadata(),
                ),
            )]
            .into(),
        )
        .inactive_sos1_constraints(
            [(
                11.into(),
                (
                    Sos1Constraint::new([0.into(), 1.into()].into()).unwrap(),
                    metadata(),
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
    assert_eq!(
        instance.inactive_one_hot_constraints()[&10.into()].1,
        metadata()
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
        .deactivate_constraint(2.into(), "duplicate".into(), [])
        .is_err());
    instance.activate_constraint(2.into()).unwrap();
    instance.activate_indicator_constraint(5.into()).unwrap();
    instance.activate_one_hot_constraint(8.into()).unwrap();
    instance.activate_sos1_constraint(11.into()).unwrap();
    assert_eq!(instance.constraints().len(), 3);
    assert_eq!(instance.active_constraints().len(), 2);
    assert_eq!(instance.constraint_context().collect_for(2.into()), context);
    instance
        .deactivate_constraint(2.into(), "again".into(), [])
        .unwrap();
    assert!(instance.inactive_constraints().contains_key(&2.into()));
    assert_eq!(instance.constraint_context().collect_for(2.into()), context);
}

#[test]
fn specialization_and_partial_evaluation_preserve_inactive_metadata() {
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
                    metadata(),
                ),
            )]
            .into(),
        )
        .inactive_indicator_constraints(
            [(
                1.into(),
                (
                    IndicatorConstraint::new(0.into(), Equality::LessThanOrEqualToZero, function),
                    metadata(),
                ),
            )]
            .into(),
        )
        .inactive_one_hot_constraints(
            [(
                2.into(),
                (
                    OneHotConstraint::new([0.into()].into()).unwrap(),
                    metadata(),
                ),
            )]
            .into(),
        )
        .inactive_sos1_constraints(
            [(
                3.into(),
                (Sos1Constraint::new([0.into()].into()).unwrap(), metadata()),
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
    assert_eq!(instance.inactive_constraints()[&0.into()].1, metadata());
    assert_eq!(
        instance.inactive_indicator_constraints()[&1.into()].1,
        metadata()
    );
    assert_eq!(
        instance.inactive_one_hot_constraints()[&2.into()].1,
        metadata()
    );
    assert_eq!(
        instance.inactive_sos1_constraints()[&3.into()].1,
        metadata()
    );
    let solution = instance
        .evaluate(&v1::State::default(), ATol::default())
        .unwrap();
    assert!(!solution.feasible());
    assert_eq!(solution.constraint_violation(0.into()), Some(3.0));
    assert_eq!(solution.indicator_constraint_violation(1.into()), Some(3.0));
}
