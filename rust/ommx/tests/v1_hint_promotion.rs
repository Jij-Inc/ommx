use ommx::{
    coeff, ATol, Constraint, ConstraintID, DecisionVariable, Function, Instance, Linear,
    LinearMonomial, Message, OneHotPromotionRequest, Sense, Sos1BigMPromotionRequest, VariableID,
};
use std::collections::BTreeMap;

const ONE_HOT_SOURCE_ID: u64 = 10;
const SOS1_SOURCE_ID: u64 = 20;

fn term(id: u64, coefficient: f64) -> Linear {
    Linear::single_term(
        LinearMonomial::Variable(VariableID::from(id)),
        ommx::Coefficient::try_from(coefficient).unwrap(),
    )
}

fn binary_pair_cardinality(first: u64, second: u64) -> Function {
    Function::from(
        ((term(first, 1.0) + term(second, 1.0)).unwrap() + Linear::from(coeff!(-1.0))).unwrap(),
    )
}

fn source_instance() -> Instance {
    Instance::new(
        Sense::Minimize,
        Function::Zero,
        (0..=3)
            .map(|id| (VariableID::from(id), DecisionVariable::binary()))
            .collect(),
        BTreeMap::from([
            (
                ConstraintID::from(ONE_HOT_SOURCE_ID),
                Constraint::equal_to_zero(binary_pair_cardinality(0, 1)),
            ),
            (
                ConstraintID::from(SOS1_SOURCE_ID),
                Constraint::less_than_or_equal_to_zero(binary_pair_cardinality(2, 3)),
            ),
        ]),
    )
    .unwrap()
}

fn valid_one_hot_hint() -> ommx::v1::OneHot {
    let mut hint = ommx::v1::OneHot::default();
    hint.constraint_id = ONE_HOT_SOURCE_ID;
    // Wire order is intentionally different from the domain set order.
    hint.decision_variables = vec![1, 0];
    hint
}

fn valid_sos1_hint() -> ommx::v1::Sos1 {
    let mut hint = ommx::v1::Sos1::default();
    hint.binary_constraint_id = SOS1_SOURCE_ID;
    // This all-binary formulation needs only its cardinality row.
    hint.decision_variables = vec![3, 2];
    hint
}

fn raw_instance_with_hints(
    one_hot_constraints: Vec<ommx::v1::OneHot>,
    sos1_constraints: Vec<ommx::v1::Sos1>,
) -> ommx::v1::Instance {
    let mut raw = ommx::v1::Instance::try_from(source_instance()).unwrap();
    let mut hints = ommx::v1::ConstraintHints::default();
    hints.one_hot_constraints = one_hot_constraints;
    hints.sos1_constraints = sos1_constraints;
    raw.constraint_hints = Some(hints);
    raw
}

#[test]
fn public_loader_promotes_valid_hints_from_real_v1_bytes() {
    let one_hot_hint = valid_one_hot_hint();
    let sos1_hint = valid_sos1_hint();
    let bytes = raw_instance_with_hints(vec![one_hot_hint.clone()], vec![sos1_hint.clone()])
        .encode_to_vec();

    let (instance, report) =
        Instance::from_v1_bytes_with_promotion(&bytes, ATol::default()).unwrap();

    assert!(!report.has_rejections());
    assert_eq!(report.one_hot_outcomes().len(), 1);
    assert_eq!(report.sos1_outcomes().len(), 1);

    let one_hot_outcome = &report.one_hot_outcomes()[0];
    assert_eq!(one_hot_outcome.index(), 0);
    assert_eq!(one_hot_outcome.hint(), &one_hot_hint);
    assert!(one_hot_outcome.error().is_none());
    let one_hot_promotion = one_hot_outcome.promotion().unwrap();
    assert_eq!(
        one_hot_promotion.relaxed_constraint_id(),
        ConstraintID::from(ONE_HOT_SOURCE_ID)
    );
    assert!(instance
        .one_hot_constraints()
        .contains_key(&one_hot_promotion.one_hot_constraint_id()));

    let sos1_outcome = &report.sos1_outcomes()[0];
    assert_eq!(sos1_outcome.index(), 0);
    assert_eq!(sos1_outcome.hint(), &sos1_hint);
    assert!(sos1_outcome.error().is_none());
    let sos1_promotion = sos1_outcome.promotion().unwrap();
    assert!(sos1_promotion
        .relaxed_constraint_ids()
        .contains(&ConstraintID::from(SOS1_SOURCE_ID)));
    assert!(instance
        .sos1_constraints()
        .contains_key(&sos1_promotion.sos1_constraint_id()));

    assert!(instance.constraints().is_empty());
    assert!(instance
        .removed_constraints()
        .contains_key(&ConstraintID::from(ONE_HOT_SOURCE_ID)));
    assert!(instance
        .removed_constraints()
        .contains_key(&ConstraintID::from(SOS1_SOURCE_ID)));
}

#[test]
fn invalid_hints_do_not_block_independent_valid_promotions() {
    let mut invalid_one_hot = ommx::v1::OneHot::default();
    invalid_one_hot.constraint_id = 999;
    invalid_one_hot.decision_variables = vec![0, 1];
    let valid_one_hot = valid_one_hot_hint();
    let mut invalid_sos1 = ommx::v1::Sos1::default();
    invalid_sos1.binary_constraint_id = 998;
    invalid_sos1.decision_variables = vec![2, 3];
    let valid_sos1 = valid_sos1_hint();
    let bytes = raw_instance_with_hints(
        vec![invalid_one_hot.clone(), valid_one_hot],
        vec![invalid_sos1.clone(), valid_sos1],
    )
    .encode_to_vec();

    let (instance, report) =
        Instance::from_v1_bytes_with_promotion(&bytes, ATol::default()).unwrap();

    assert!(report.has_rejections());
    assert_eq!(report.one_hot_outcomes().len(), 2);
    assert_eq!(report.sos1_outcomes().len(), 2);

    let invalid_one_hot_outcome = &report.one_hot_outcomes()[0];
    assert_eq!(invalid_one_hot_outcome.index(), 0);
    assert_eq!(invalid_one_hot_outcome.hint(), &invalid_one_hot);
    assert!(!invalid_one_hot_outcome.is_promoted());
    assert!(invalid_one_hot_outcome.promotion().is_none());
    assert!(invalid_one_hot_outcome.error().is_some());
    assert!(report.one_hot_outcomes()[1].is_promoted());

    let invalid_sos1_outcome = &report.sos1_outcomes()[0];
    assert_eq!(invalid_sos1_outcome.index(), 0);
    assert_eq!(invalid_sos1_outcome.hint(), &invalid_sos1);
    assert!(!invalid_sos1_outcome.is_promoted());
    assert!(invalid_sos1_outcome.promotion().is_none());
    assert!(invalid_sos1_outcome.error().is_some());
    assert!(report.sos1_outcomes()[1].is_promoted());

    assert_eq!(instance.one_hot_constraints().len(), 1);
    assert_eq!(instance.sos1_constraints().len(), 1);
    assert!(instance.constraints().is_empty());
}

#[test]
fn ordinary_v1_loader_still_ignores_valid_hints() {
    let bytes = raw_instance_with_hints(vec![valid_one_hot_hint()], vec![valid_sos1_hint()])
        .encode_to_vec();

    let instance = Instance::from_v1_bytes(&bytes).unwrap();

    assert_eq!(instance.constraints().len(), 2);
    assert!(instance
        .constraints()
        .contains_key(&ConstraintID::from(ONE_HOT_SOURCE_ID)));
    assert!(instance
        .constraints()
        .contains_key(&ConstraintID::from(SOS1_SOURCE_ID)));
    assert!(instance.removed_constraints().is_empty());
    assert!(instance.one_hot_constraints().is_empty());
    assert!(instance.sos1_constraints().is_empty());
}

#[test]
fn promotion_loader_without_hints_matches_the_ordinary_loader() {
    let mut raw = ommx::v1::Instance::try_from(source_instance()).unwrap();
    raw.constraint_hints = None;
    let bytes = raw.encode_to_vec();

    let expected = Instance::from_v1_bytes(&bytes).unwrap();
    let (actual, report) = Instance::from_v1_bytes_with_promotion(&bytes, ATol::default()).unwrap();

    assert_eq!(actual, expected);
    assert!(report.one_hot_outcomes().is_empty());
    assert!(report.sos1_outcomes().is_empty());
    assert!(!report.has_rejections());
}

#[test]
fn promotion_loader_keeps_decode_and_base_parse_failures_as_outer_errors() {
    assert!(Instance::from_v1_bytes_with_promotion(&[0xff], ATol::default()).is_err());

    let mut hints = ommx::v1::ConstraintHints::default();
    hints.one_hot_constraints.push(valid_one_hot_hint());
    hints.sos1_constraints.push(valid_sos1_hint());
    let mut invalid_base = ommx::v1::Instance::default();
    invalid_base.constraint_hints = Some(hints);
    assert!(
        Instance::from_v1_bytes_with_promotion(&invalid_base.encode_to_vec(), ATol::default())
            .is_err()
    );
}

#[test]
fn individual_hint_requests_remain_independently_applicable() {
    let one_hot_hint = valid_one_hot_hint();
    let sos1_hint = valid_sos1_hint();
    let bytes = raw_instance_with_hints(vec![], vec![]).encode_to_vec();
    let original = Instance::from_v1_bytes(&bytes).unwrap();

    let request = OneHotPromotionRequest::from_v1_hint(&one_hot_hint).unwrap();
    assert_eq!(
        request.source_constraint_id,
        ConstraintID::from(ONE_HOT_SOURCE_ID)
    );
    assert_eq!(
        request.variables,
        [VariableID::from(0), VariableID::from(1)]
            .into_iter()
            .collect()
    );
    let mut one_hot_only = original.clone();
    let _ = one_hot_only.promote_one_hot(&request).unwrap();
    assert_eq!(one_hot_only.one_hot_constraints().len(), 1);
    assert!(one_hot_only
        .constraints()
        .contains_key(&ConstraintID::from(SOS1_SOURCE_ID)));

    let mut sos1_only = original;
    let request = Sos1BigMPromotionRequest::from_v1_hint(&sos1_only, &sos1_hint).unwrap();
    assert_eq!(
        request.cardinality_constraint,
        ConstraintID::from(SOS1_SOURCE_ID)
    );
    let _ = sos1_only
        .promote_sos1_big_m(&request, ATol::default())
        .unwrap();
    assert_eq!(sos1_only.sos1_constraints().len(), 1);
    assert!(sos1_only
        .constraints()
        .contains_key(&ConstraintID::from(ONE_HOT_SOURCE_ID)));
}

#[test]
fn conflicting_individually_valid_hints_are_not_resolved_first_wins() {
    let first = valid_one_hot_hint();
    let mut second = first.clone();
    second.decision_variables.reverse();
    let bytes =
        raw_instance_with_hints(vec![first.clone(), second.clone()], vec![valid_sos1_hint()])
            .encode_to_vec();

    // Both claims are valid when considered independently.
    let original = Instance::from_v1_bytes(&bytes).unwrap();
    for hint in [&first, &second] {
        let request = OneHotPromotionRequest::from_v1_hint(hint).unwrap();
        let mut independently_promoted = original.clone();
        let _ = independently_promoted.promote_one_hot(&request).unwrap();
    }

    let (instance, report) =
        Instance::from_v1_bytes_with_promotion(&bytes, ATol::default()).unwrap();

    assert!(report.has_rejections());
    assert_eq!(report.one_hot_outcomes().len(), 2);
    for (index, outcome) in report.one_hot_outcomes().iter().enumerate() {
        assert_eq!(outcome.index(), index);
        assert!(!outcome.is_promoted());
        assert!(outcome.promotion().is_none());
        assert!(outcome.error().is_some());
    }
    assert!(instance
        .constraints()
        .contains_key(&ConstraintID::from(ONE_HOT_SOURCE_ID)));
    assert!(instance.one_hot_constraints().is_empty());

    // The unrelated SOS1 hint still succeeds despite the OneHot conflict.
    assert!(report.sos1_outcomes()[0].is_promoted());
    assert!(!instance
        .constraints()
        .contains_key(&ConstraintID::from(SOS1_SOURCE_ID)));
    assert_eq!(instance.sos1_constraints().len(), 1);
}

#[test]
fn conflicting_sos1_hints_are_both_rejected_without_blocking_one_hot() {
    let first = valid_sos1_hint();
    let mut second = first.clone();
    second.decision_variables.reverse();
    let bytes = raw_instance_with_hints(
        vec![valid_one_hot_hint()],
        vec![first.clone(), second.clone()],
    )
    .encode_to_vec();

    let original = Instance::from_v1_bytes(&bytes).unwrap();
    for hint in [&first, &second] {
        let request = Sos1BigMPromotionRequest::from_v1_hint(&original, hint).unwrap();
        let mut independently_promoted = original.clone();
        let _ = independently_promoted
            .promote_sos1_big_m(&request, ATol::default())
            .unwrap();
    }

    let (instance, report) =
        Instance::from_v1_bytes_with_promotion(&bytes, ATol::default()).unwrap();

    for (index, outcome) in report.sos1_outcomes().iter().enumerate() {
        assert_eq!(outcome.index(), index);
        assert!(!outcome.is_promoted());
        assert!(outcome.promotion().is_none());
        let error = outcome.error().unwrap().to_string();
        assert!(error.contains("consumed regular rows"));
        assert!(error.contains("ConstraintID(20)"));
    }
    assert!(instance
        .constraints()
        .contains_key(&ConstraintID::from(SOS1_SOURCE_ID)));
    assert!(instance.sos1_constraints().is_empty());

    assert!(report.one_hot_outcomes()[0].is_promoted());
    assert!(!instance
        .constraints()
        .contains_key(&ConstraintID::from(ONE_HOT_SOURCE_ID)));
    assert_eq!(instance.one_hot_constraints().len(), 1);
}
