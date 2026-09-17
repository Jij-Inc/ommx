use ommx::{
    coeff, v1, ATol, Bound, Constraint, ConstraintID, DecisionVariable, Evaluate, Function,
    Instance, InstanceParameters, Kind, Linear, LinearMonomial, Message,
    OneHotPromotionBatchRejected, OneHotPromotionRequest, Sense, Sos1BigMPromotionBatchRejected,
    Sos1BigMPromotionRequest, Sos1BigMSelectorClaim,
};
use proptest::prelude::*;
use std::collections::BTreeMap;

fn term(id: u64, value: f64) -> Linear {
    Linear::single_term(
        LinearMonomial::Variable(id.into()),
        value.try_into().unwrap(),
    )
}

fn source(upper: f64) -> Instance {
    Instance::new(
        Sense::Minimize,
        Function::from((term(1, 2.0) + term(2, 1.0)).unwrap()),
        BTreeMap::from([
            (0.into(), DecisionVariable::binary()),
            (1.into(), DecisionVariable::binary()),
            (
                2.into(),
                DecisionVariable::new(
                    Kind::Continuous,
                    Bound::new(-2.0, upper).unwrap(),
                    ATol::default(),
                )
                .unwrap(),
            ),
            (3.into(), DecisionVariable::binary()),
        ]),
        BTreeMap::from([
            (
                10.into(),
                Constraint::equal_to_zero(Function::from(
                    ((term(0, 1.0) + term(1, 1.0)).unwrap() + Linear::from(coeff!(-1.0))).unwrap(),
                )),
            ),
            (
                20.into(),
                Constraint::less_than_or_equal_to_zero(Function::from(
                    ((term(0, 1.0) + term(3, 1.0)).unwrap() + Linear::from(coeff!(-1.0))).unwrap(),
                )),
            ),
            (
                21.into(),
                Constraint::less_than_or_equal_to_zero(Function::from(
                    (term(2, 1.0) + term(3, -3.0)).unwrap(),
                )),
            ),
            (
                22.into(),
                Constraint::less_than_or_equal_to_zero(Function::from(
                    (term(2, -1.0) + term(3, -2.0)).unwrap(),
                )),
            ),
        ]),
    )
    .unwrap()
}

fn one_hot_request() -> OneHotPromotionRequest {
    [ConstraintID::from(10)].into()
}

fn sos1_request() -> Sos1BigMPromotionRequest {
    BTreeMap::from([(
        20.into(),
        BTreeMap::from([
            (0.into(), Sos1BigMSelectorClaim::Reused),
            (
                2.into(),
                Sos1BigMSelectorClaim::Fresh {
                    selector: 3.into(),
                    upper_link: Some(21.into()),
                    lower_link: Some(22.into()),
                },
            ),
        ]),
    )])
}

fn hints(instance: &mut Instance) -> v1::ConstraintHints {
    let mut hints = instance
        .plan_promote_one_hot(&one_hot_request())
        .into_v1_hints()
        .unwrap();
    hints.sos1_constraints = instance
        .plan_promote_sos1_big_m(&sos1_request())
        .into_v1_hints()
        .unwrap()
        .sos1_constraints;
    hints
}

#[test]
fn exporting_both_plans_preserves_the_complete_regular_model_and_ids() {
    let mut instance = source(3.0);
    let original = instance.clone();
    let hints = hints(&mut instance);
    assert_eq!(instance, original);
    assert_eq!(hints.one_hot_constraints[0].constraint_id, 10);
    assert_eq!(hints.one_hot_constraints[0].decision_variables, [0, 1]);
    assert_eq!(hints.sos1_constraints[0].binary_constraint_id, 20);
    assert_eq!(hints.sos1_constraints[0].big_m_constraint_ids, [21, 22]);
    assert_eq!(hints.sos1_constraints[0].decision_variables, [0, 2]);
    let mut raw = instance.into_v1_with_hints(hints.clone()).unwrap();
    assert_eq!(raw.constraint_hints.take(), Some(hints));
    assert_eq!(raw, v1::Instance::try_from(original).unwrap());
}

#[test]
fn direct_promotion_and_hint_import_preserve_projected_feasibility_and_objective() {
    let mut regular = source(3.0);
    let hints = hints(&mut regular);
    let bytes = regular
        .clone()
        .into_v1_with_hints(hints)
        .unwrap()
        .encode_to_vec();
    let ignored = Instance::from_v1_bytes(&bytes).unwrap();
    assert_eq!(ignored, regular);
    let (imported, report) = Instance::from_v1_bytes_with_promotion(&bytes).unwrap();
    assert!(report
        .one_hot_outcomes()
        .iter()
        .all(|entry| entry.is_promoted()));
    assert!(report
        .sos1_outcomes()
        .iter()
        .all(|entry| entry.is_promoted()));
    let mut direct = regular.clone();
    direct
        .plan_promote_one_hot(&one_hot_request())
        .apply_if_fully_valid()
        .unwrap();
    direct
        .plan_promote_sos1_big_m(&sos1_request())
        .apply_if_fully_valid()
        .unwrap();
    for first in [0.0, 1.0] {
        for second in [0.0, 1.0] {
            for member in [-2.0, -1.0, 0.0, 1.0, 3.0] {
                let state = v1::State::from_iter([(0, first), (1, second), (2, member)]);
                let projected = [0.0, 1.0].into_iter().any(|selector| {
                    let mut extended = state.clone();
                    extended.entries.insert(3, selector);
                    regular
                        .evaluate(&extended, ATol::default())
                        .unwrap()
                        .feasible_relaxed()
                });
                for promoted in [&direct, &imported] {
                    let result = promoted.evaluate(&state, ATol::default()).unwrap();
                    assert_eq!(result.feasible_relaxed(), projected);
                    assert_eq!(*result.objective(), 2.0 * second + member);
                }
            }
        }
    }
}

#[test]
fn rejected_plans_export_no_hints_and_apply_nothing() {
    let mut instance = source(3.0);
    let original = instance.clone();
    let mut request = one_hot_request();
    request.insert(999.into());
    let plan = instance.plan_promote_one_hot(&request);
    assert!(!plan.is_fully_valid());
    assert_eq!(
        plan.rejections().map(|(id, _)| id).collect::<Vec<_>>(),
        [999.into()]
    );
    let error = plan.into_v1_hints().unwrap_err();
    let rejected = error
        .downcast_ref::<OneHotPromotionBatchRejected>()
        .unwrap();
    assert_eq!(rejected.request_count(), 2);
    assert_eq!(rejected.rejections().count(), 1);
    assert_eq!(instance, original);
    assert!(instance
        .plan_promote_one_hot(&request)
        .apply_if_fully_valid()
        .is_err());
    assert_eq!(instance, original);
    let mut request = sos1_request();
    request.insert(999.into(), BTreeMap::new());
    let error = instance
        .plan_promote_sos1_big_m(&request)
        .into_v1_hints()
        .unwrap_err();
    assert!(error.is::<Sos1BigMPromotionBatchRejected>());
    assert_eq!(instance, original);
}

#[test]
fn detached_hints_are_revalidated_against_the_final_instance() {
    let mut instance = source(3.0);
    let valid = hints(&mut instance);
    let mut invalid = valid.clone();
    invalid.one_hot_constraints[0].decision_variables = vec![0];
    assert!(instance.clone().into_v1_with_hints(invalid).is_err());
    let mut invalid = valid.clone();
    invalid.sos1_constraints[0].big_m_constraint_ids.push(999);
    assert!(instance.clone().into_v1_with_hints(invalid).is_err());
    let mut invalid = valid.clone();
    invalid.sos1_constraints[0].decision_variables.push(2);
    assert!(instance.clone().into_v1_with_hints(invalid).is_err());
    let mut invalid = valid.clone();
    invalid.sos1_constraints[0].big_m_constraint_ids.push(21);
    assert!(instance.clone().into_v1_with_hints(invalid).is_err());
    assert!(source(4.0).into_v1_with_hints(valid).is_err());
}

#[test]
fn identical_repeated_hints_and_reordered_ids_are_preserved() {
    let mut instance = source(3.0);
    let mut hints = hints(&mut instance);
    hints.one_hot_constraints[0].decision_variables.reverse();
    hints.sos1_constraints[0].decision_variables.reverse();
    hints.sos1_constraints[0].big_m_constraint_ids.reverse();
    hints
        .one_hot_constraints
        .push(hints.one_hot_constraints[0].clone());
    hints
        .sos1_constraints
        .push(hints.sos1_constraints[0].clone());
    let raw = instance.into_v1_with_hints(hints.clone()).unwrap();
    assert_eq!(raw.constraint_hints, Some(hints));
}

#[test]
fn empty_plans_release_the_borrow_without_effects() {
    let mut instance = source(3.0);
    let before = instance.clone();
    assert_eq!(
        instance
            .plan_promote_one_hot(&Default::default())
            .into_v1_hints()
            .unwrap(),
        v1::ConstraintHints::default()
    );
    assert_eq!(
        instance
            .plan_promote_sos1_big_m(&Default::default())
            .into_v1_hints()
            .unwrap(),
        v1::ConstraintHints::default()
    );
    assert_eq!(instance, before);
}

#[test]
fn ambiguous_zero_bound_selectors_are_rejected_without_changing_the_model() {
    let zero = DecisionVariable::new(
        Kind::Continuous,
        Bound::new(0.0, 0.0).unwrap(),
        ATol::default(),
    )
    .unwrap();
    let mut instance = Instance::new(
        Sense::Minimize,
        Function::Zero,
        BTreeMap::from([
            (0.into(), zero.clone()),
            (1.into(), zero),
            (10.into(), DecisionVariable::binary()),
            (11.into(), DecisionVariable::binary()),
        ]),
        BTreeMap::from([(
            20.into(),
            Constraint::less_than_or_equal_to_zero(Function::from(
                ((term(10, 1.0) + term(11, 1.0)).unwrap() + Linear::from(coeff!(-1.0))).unwrap(),
            )),
        )]),
    )
    .unwrap();
    let request = BTreeMap::from([(
        20.into(),
        BTreeMap::from([
            (
                0.into(),
                Sos1BigMSelectorClaim::Fresh {
                    selector: 10.into(),
                    upper_link: None,
                    lower_link: None,
                },
            ),
            (
                1.into(),
                Sos1BigMSelectorClaim::Fresh {
                    selector: 11.into(),
                    upper_link: None,
                    lower_link: None,
                },
            ),
        ]),
    )]);
    let before = instance.clone();
    let plan = instance.plan_promote_sos1_big_m(&request);
    assert!(plan.is_fully_valid());
    assert!(plan
        .into_v1_hints()
        .unwrap_err()
        .to_string()
        .contains("ambiguous"));
    assert_eq!(instance, before);
}

#[test]
fn dropping_one_hot_plan_preserves_source_and_native_v1_export_still_fails() {
    let mut instance = source(3.0);
    let original = instance.clone();
    drop(instance.plan_promote_one_hot(&one_hot_request()));
    assert_eq!(instance, original);
    instance
        .plan_promote_one_hot(&one_hot_request())
        .apply_if_fully_valid()
        .unwrap();
    assert!(instance.into_v1_with_hints(Default::default()).is_err());
}

proptest! {
    #[test]
    fn one_hot_plan_hints_round_trip_arbitrary_source_ids_and_members(
        members in prop::collection::btree_set(0_u64..32, 1..12),
        source_id in 0_u64..1000,
        scale in prop::sample::select(vec![-3.0, -1.0, 0.5, 2.0]),
    ) {
        let equality = members.iter().fold(Linear::from(ommx::Coefficient::try_from(-scale).unwrap()), |row, &id| {
            (row + term(id, scale)).unwrap()
        });
        let mut source = Instance::new(Sense::Minimize, Function::Zero,
            members.iter().map(|&id| (id.into(), DecisionVariable::binary())).collect(),
            BTreeMap::from([(source_id.into(), Constraint::equal_to_zero(equality.into()))]),
        ).unwrap();
        let original = source.clone();
        let hints = source.plan_promote_one_hot(&[source_id.into()].into()).into_v1_hints().unwrap();
        prop_assert_eq!(&source, &original);
        let bytes = source.into_v1_with_hints(hints).unwrap().encode_to_vec();
        prop_assert_eq!(Instance::from_v1_bytes(&bytes).unwrap(), original);
        let (promoted, report) = Instance::from_v1_bytes_with_promotion(&bytes).unwrap();
        prop_assert!(report.one_hot_outcomes()[0].is_promoted());
        prop_assert_eq!(promoted.one_hot_constraints().values().next().unwrap().variables.iter()
            .map(|id| id.into_inner()).collect::<std::collections::BTreeSet<_>>(), members);
    }

    #[test]
    fn empty_hints_preserve_v1_compatible_instances(
        source in Instance::arbitrary_with(InstanceParameters::v1_compatible()),
    ) {
        let raw = source.clone().into_v1_with_hints(Default::default()).unwrap();
        prop_assert_eq!(&raw, &v1::Instance::try_from(source.clone()).unwrap());
        prop_assert_eq!(Instance::from_v1_bytes(&raw.encode_to_vec()).unwrap(), source);
    }
}
