use ommx::{
    coeff, ATol, Bound, Constraint, ConstraintID, DecisionVariable, Function, Instance, Kind,
    Linear, LinearMonomial, Message, OneHotConstraintID, OneHotPromotionRequest, ParseError,
    RawParseError, Sense, Sos1BigMPromotionRequest, VariableID,
};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error as _;

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

fn shared_member_source_instance() -> Instance {
    Instance::new(
        Sense::Minimize,
        Function::Zero,
        (0..=2)
            .map(|id| (VariableID::from(id), DecisionVariable::binary()))
            .collect(),
        BTreeMap::from([
            (
                ConstraintID::from(ONE_HOT_SOURCE_ID),
                Constraint::equal_to_zero(binary_pair_cardinality(0, 1)),
            ),
            (
                ConstraintID::from(SOS1_SOURCE_ID),
                Constraint::less_than_or_equal_to_zero(binary_pair_cardinality(0, 2)),
            ),
        ]),
    )
    .unwrap()
}

fn fresh_selector_source_instance(one_hot_members: (u64, u64)) -> Instance {
    Instance::new(
        Sense::Minimize,
        Function::Zero,
        BTreeMap::from([
            (VariableID::from(0), DecisionVariable::binary()),
            (
                VariableID::from(1),
                DecisionVariable::new(
                    Kind::Integer,
                    Bound::new(-2.0, 3.0).unwrap(),
                    Default::default(),
                )
                .unwrap(),
            ),
            (VariableID::from(10), DecisionVariable::binary()),
            (VariableID::from(11), DecisionVariable::binary()),
            (VariableID::from(12), DecisionVariable::binary()),
        ]),
        BTreeMap::from([
            (
                ConstraintID::from(50),
                Constraint::equal_to_zero(binary_pair_cardinality(
                    one_hot_members.0,
                    one_hot_members.1,
                )),
            ),
            (
                ConstraintID::from(100),
                Constraint::less_than_or_equal_to_zero(Function::from(
                    (term(1, 1.0) + term(10, -3.0)).unwrap(),
                )),
            ),
            (
                ConstraintID::from(101),
                Constraint::less_than_or_equal_to_zero(Function::from(
                    (term(1, -1.0) + term(10, -2.0)).unwrap(),
                )),
            ),
            (
                ConstraintID::from(102),
                Constraint::less_than_or_equal_to_zero(binary_pair_cardinality(0, 10)),
            ),
        ]),
    )
    .unwrap()
}

fn three_sos1_source_instance() -> Instance {
    Instance::new(
        Sense::Minimize,
        Function::Zero,
        (0..=5)
            .map(|id| (VariableID::from(id), DecisionVariable::binary()))
            .collect(),
        BTreeMap::from([
            (
                ConstraintID::from(20),
                Constraint::less_than_or_equal_to_zero(binary_pair_cardinality(0, 1)),
            ),
            (
                ConstraintID::from(21),
                Constraint::less_than_or_equal_to_zero(binary_pair_cardinality(2, 3)),
            ),
            (
                ConstraintID::from(22),
                Constraint::less_than_or_equal_to_zero(binary_pair_cardinality(4, 5)),
            ),
        ]),
    )
    .unwrap()
}

fn valid_one_hot_hint() -> ommx::v1::OneHot {
    one_hot_hint(ONE_HOT_SOURCE_ID, vec![1, 0])
}

fn valid_sos1_hint() -> ommx::v1::Sos1 {
    sos1_hint(SOS1_SOURCE_ID, vec![3, 2])
}

fn one_hot_hint(constraint_id: u64, decision_variables: Vec<u64>) -> ommx::v1::OneHot {
    let mut hint = ommx::v1::OneHot::default();
    hint.constraint_id = constraint_id;
    hint.decision_variables = decision_variables;
    hint
}

fn sos1_hint(binary_constraint_id: u64, decision_variables: Vec<u64>) -> ommx::v1::Sos1 {
    let mut hint = ommx::v1::Sos1::default();
    hint.binary_constraint_id = binary_constraint_id;
    hint.decision_variables = decision_variables;
    hint
}

fn raw_instance_from(
    instance: Instance,
    one_hot_constraints: Vec<ommx::v1::OneHot>,
    sos1_constraints: Vec<ommx::v1::Sos1>,
) -> ommx::v1::Instance {
    let mut raw = ommx::v1::Instance::try_from(instance).unwrap();
    let mut hints = ommx::v1::ConstraintHints::default();
    hints.one_hot_constraints = one_hot_constraints;
    hints.sos1_constraints = sos1_constraints;
    raw.constraint_hints = Some(hints);
    raw
}

fn raw_instance_with_hints(
    one_hot_constraints: Vec<ommx::v1::OneHot>,
    sos1_constraints: Vec<ommx::v1::Sos1>,
) -> ommx::v1::Instance {
    raw_instance_from(source_instance(), one_hot_constraints, sos1_constraints)
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
    let one_hot_constraint_id = one_hot_outcome.one_hot_constraint_id().unwrap();
    assert!(instance
        .one_hot_constraints()
        .contains_key(&one_hot_constraint_id));

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
    let invalid_one_hot = one_hot_hint(999, vec![0, 1]);
    let invalid_sos1 = sos1_hint(998, vec![2, 3]);
    let bytes = raw_instance_with_hints(
        vec![invalid_one_hot.clone(), valid_one_hot_hint()],
        vec![invalid_sos1.clone(), valid_sos1_hint()],
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
    assert!(invalid_one_hot_outcome.one_hot_constraint_id().is_none());
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
fn promotion_loader_preserves_parse_error_as_the_outer_error() {
    let ordinary_decode_error = Instance::from_v1_bytes(&[0x80]).unwrap_err();
    let decode_error =
        Instance::from_v1_bytes_with_promotion(&[0x80], ATol::default()).unwrap_err();
    assert_eq!(
        format!("{decode_error:#}"),
        format!("{ordinary_decode_error:#}")
    );
    let decode_parse_error = decode_error.downcast_ref::<ParseError>().unwrap();
    let ordinary_decode_parse_error = ordinary_decode_error.downcast_ref::<ParseError>().unwrap();
    assert_eq!(decode_parse_error.context.len(), 1);
    assert_eq!(
        decode_parse_error.context[0].message,
        ordinary_decode_parse_error.context[0].message
    );
    assert_eq!(
        decode_parse_error.context[0].field,
        ordinary_decode_parse_error.context[0].field
    );
    assert!(matches!(
        decode_parse_error
            .source()
            .and_then(|source| source.downcast_ref::<RawParseError>()),
        Some(RawParseError::DecodeError(_))
    ));

    let mut hints = ommx::v1::ConstraintHints::default();
    hints.one_hot_constraints.push(valid_one_hot_hint());
    hints.sos1_constraints.push(valid_sos1_hint());
    let mut invalid_base = ommx::v1::Instance::default();
    invalid_base.constraint_hints = Some(hints);
    let invalid_bytes = invalid_base.encode_to_vec();
    let ordinary_semantic_error = Instance::from_v1_bytes(&invalid_bytes).unwrap_err();
    let semantic_error =
        Instance::from_v1_bytes_with_promotion(&invalid_bytes, ATol::default()).unwrap_err();
    assert_eq!(
        format!("{semantic_error:#}"),
        format!("{ordinary_semantic_error:#}")
    );
    let semantic_parse_error = semantic_error.downcast_ref::<ParseError>().unwrap();
    let ordinary_semantic_parse_error = ordinary_semantic_error
        .downcast_ref::<ParseError>()
        .unwrap();
    assert_eq!(
        semantic_parse_error.context.len(),
        ordinary_semantic_parse_error.context.len()
    );
    for (actual, ordinary) in semantic_parse_error
        .context
        .iter()
        .zip(&ordinary_semantic_parse_error.context)
    {
        assert_eq!(actual.message, ordinary.message);
        assert_eq!(actual.field, ordinary.field);
    }
    assert!(matches!(
        semantic_parse_error
            .source()
            .and_then(|source| source.downcast_ref::<RawParseError>()),
        Some(RawParseError::MissingField { .. })
    ));
}

#[test]
fn individual_hint_requests_remain_independently_applicable() {
    let one_hot_hint = valid_one_hot_hint();
    let one_hot_request = OneHotPromotionRequest::from(&one_hot_hint);
    assert_eq!(
        one_hot_request,
        BTreeSet::from([ConstraintID::from(ONE_HOT_SOURCE_ID)])
    );

    let mut one_hot_only = source_instance();
    let one_hot_results = one_hot_only.promote_one_hot(&one_hot_request);
    assert!(one_hot_results[&ConstraintID::from(ONE_HOT_SOURCE_ID)].is_ok());
    assert_eq!(one_hot_only.one_hot_constraints().len(), 1);
    assert!(one_hot_only
        .constraints()
        .contains_key(&ConstraintID::from(SOS1_SOURCE_ID)));

    let mut sos1_only = source_instance();
    let sos1_request =
        Sos1BigMPromotionRequest::from_v1_hint(&sos1_only, &valid_sos1_hint()).unwrap();
    assert_eq!(
        sos1_request.cardinality_constraint,
        ConstraintID::from(SOS1_SOURCE_ID)
    );
    let outcomes =
        sos1_only.promote_sos1_big_m(std::slice::from_ref(&sos1_request), ATol::default());
    assert_eq!(outcomes.len(), 1);
    let _ = outcomes
        .into_iter()
        .next()
        .expect("one request has one aligned result")
        .unwrap();
    assert_eq!(sos1_only.sos1_constraints().len(), 1);
    assert!(sos1_only
        .constraints()
        .contains_key(&ConstraintID::from(ONE_HOT_SOURCE_ID)));
}

#[test]
fn repeated_valid_one_hot_hints_share_one_promotion_and_ignore_advisory_members() {
    let first = one_hot_hint(ONE_HOT_SOURCE_ID, vec![]);
    let second = one_hot_hint(ONE_HOT_SOURCE_ID, vec![999, 999, 42]);
    let bytes =
        raw_instance_with_hints(vec![first.clone(), second.clone()], vec![]).encode_to_vec();

    let (instance, report) =
        Instance::from_v1_bytes_with_promotion(&bytes, ATol::default()).unwrap();

    assert!(!report.has_rejections());
    assert_eq!(report.one_hot_outcomes().len(), 2);
    assert_eq!(report.one_hot_outcomes()[0].hint(), &first);
    assert_eq!(report.one_hot_outcomes()[1].hint(), &second);
    let first_id = report.one_hot_outcomes()[0]
        .one_hot_constraint_id()
        .unwrap();
    let second_id = report.one_hot_outcomes()[1]
        .one_hot_constraint_id()
        .unwrap();
    assert_eq!(first_id, second_id);
    assert_eq!(instance.one_hot_constraints().len(), 1);
    assert_eq!(
        instance.one_hot_constraints()[&first_id].variables,
        [VariableID::from(0), VariableID::from(1)]
            .into_iter()
            .collect()
    );
    assert!(!instance
        .constraints()
        .contains_key(&ConstraintID::from(ONE_HOT_SOURCE_ID)));
}

#[test]
fn maximum_regular_source_id_allocates_from_the_empty_one_hot_namespace() {
    let source_id = ConstraintID::from(u64::MAX);
    let source = Instance::new(
        Sense::Minimize,
        Function::Zero,
        (0..=1)
            .map(|id| (VariableID::from(id), DecisionVariable::binary()))
            .collect(),
        BTreeMap::from([(
            source_id,
            Constraint::equal_to_zero(binary_pair_cardinality(0, 1)),
        )]),
    )
    .unwrap();
    let hint = one_hot_hint(source_id.into_inner(), vec![0, 1]);
    let bytes = raw_instance_from(source, vec![hint], vec![]).encode_to_vec();

    let (instance, report) =
        Instance::from_v1_bytes_with_promotion(&bytes, ATol::default()).unwrap();

    assert!(!report.has_rejections());
    assert_eq!(
        report.one_hot_outcomes()[0].one_hot_constraint_id(),
        Some(OneHotConstraintID::from(0))
    );
    assert!(instance.removed_constraints().contains_key(&source_id));
    assert!(instance
        .one_hot_constraints()
        .contains_key(&OneHotConstraintID::from(0)));
}

#[test]
fn repeated_invalid_one_hot_hints_retain_owned_errors_for_each_occurrence() {
    let first = one_hot_hint(999, vec![]);
    let second = one_hot_hint(999, vec![0, 1]);
    let bytes = raw_instance_with_hints(vec![first, second], vec![]).encode_to_vec();

    let (instance, report) =
        Instance::from_v1_bytes_with_promotion(&bytes, ATol::default()).unwrap();
    let (one_hot_outcomes, sos1_outcomes) = report.into_parts();
    assert!(sos1_outcomes.is_empty());

    let mut outcomes = one_hot_outcomes.into_iter();
    let first_error = match outcomes.next().unwrap() {
        ommx::V1OneHotHintPromotionOutcome::Rejected { index, error, .. } => {
            assert_eq!(index, 0);
            error
        }
        outcome => panic!("unexpected outcome: {outcome:?}"),
    };
    let second_error = match outcomes.next().unwrap() {
        ommx::V1OneHotHintPromotionOutcome::Rejected { index, error, .. } => {
            assert_eq!(index, 1);
            error
        }
        outcome => panic!("unexpected outcome: {outcome:?}"),
    };
    assert!(first_error.to_string().contains("was not found"));
    assert!(second_error.to_string().contains("was not found"));
    assert_eq!(instance.constraints().len(), 2);
    assert!(instance.one_hot_constraints().is_empty());
}

#[test]
fn equality_row_overlap_rejects_sos1_claim_without_poisoning_one_hot() {
    let sos1_claiming_equality = sos1_hint(ONE_HOT_SOURCE_ID, vec![0, 1]);
    let bytes = raw_instance_with_hints(vec![valid_one_hot_hint()], vec![sos1_claiming_equality])
        .encode_to_vec();

    let (instance, report) =
        Instance::from_v1_bytes_with_promotion(&bytes, ATol::default()).unwrap();

    assert!(report.one_hot_outcomes()[0].is_promoted());
    assert!(!report.sos1_outcomes()[0].is_promoted());
    assert_eq!(instance.one_hot_constraints().len(), 1);
    assert!(instance.sos1_constraints().is_empty());
    assert!(!instance
        .constraints()
        .contains_key(&ConstraintID::from(ONE_HOT_SOURCE_ID)));
    assert!(instance
        .constraints()
        .contains_key(&ConstraintID::from(SOS1_SOURCE_ID)));
}

#[test]
fn inequality_row_overlap_rejects_one_hot_claim_without_poisoning_sos1() {
    let one_hot_claiming_inequality = one_hot_hint(SOS1_SOURCE_ID, vec![0, 1]);
    let bytes = raw_instance_with_hints(vec![one_hot_claiming_inequality], vec![valid_sos1_hint()])
        .encode_to_vec();

    let (instance, report) =
        Instance::from_v1_bytes_with_promotion(&bytes, ATol::default()).unwrap();

    assert!(!report.one_hot_outcomes()[0].is_promoted());
    assert!(report.sos1_outcomes()[0].is_promoted());
    assert!(instance.one_hot_constraints().is_empty());
    assert_eq!(instance.sos1_constraints().len(), 1);
    assert!(instance
        .constraints()
        .contains_key(&ConstraintID::from(ONE_HOT_SOURCE_ID)));
    assert!(!instance
        .constraints()
        .contains_key(&ConstraintID::from(SOS1_SOURCE_ID)));
}

#[test]
fn one_hot_and_sos1_promotions_may_share_an_ordinary_member() {
    let sos1_hint = sos1_hint(SOS1_SOURCE_ID, vec![2, 0]);
    let bytes = raw_instance_from(
        shared_member_source_instance(),
        vec![valid_one_hot_hint()],
        vec![sos1_hint],
    )
    .encode_to_vec();

    let (instance, report) =
        Instance::from_v1_bytes_with_promotion(&bytes, ATol::default()).unwrap();

    assert!(!report.has_rejections());
    assert!(report.one_hot_outcomes()[0].is_promoted());
    assert!(report.sos1_outcomes()[0].is_promoted());
    assert_eq!(instance.one_hot_constraints().len(), 1);
    assert_eq!(instance.sos1_constraints().len(), 1);
    assert!(instance.constraints().is_empty());

    let one_hot_id = report.one_hot_outcomes()[0]
        .one_hot_constraint_id()
        .unwrap();
    let sos1_promotion = report.sos1_outcomes()[0].promotion().unwrap();
    assert!(instance.one_hot_constraints()[&one_hot_id]
        .variables
        .contains(&VariableID::from(0)));
    assert!(sos1_promotion.members().contains(&VariableID::from(0)));
}

#[test]
fn one_hot_source_usage_rejects_an_overlapping_sos1_fresh_selector() {
    let one_hot_hint = one_hot_hint(50, vec![10, 11]);
    let mut sos1_hint = sos1_hint(102, vec![0, 1]);
    sos1_hint.big_m_constraint_ids = vec![100, 101];
    let bytes = raw_instance_from(
        fresh_selector_source_instance((10, 11)),
        vec![one_hot_hint],
        vec![sos1_hint],
    )
    .encode_to_vec();

    let (instance, report) =
        Instance::from_v1_bytes_with_promotion(&bytes, ATol::default()).unwrap();

    assert!(report.one_hot_outcomes()[0].is_promoted());
    assert!(!report.sos1_outcomes()[0].is_promoted());
    assert!(report.sos1_outcomes()[0]
        .error()
        .unwrap()
        .to_string()
        .contains("active solver input"));
    assert_eq!(instance.one_hot_constraints().len(), 1);
    assert!(instance.sos1_constraints().is_empty());
    assert!(!instance.constraints().contains_key(&ConstraintID::from(50)));
    for id in [100, 101, 102] {
        assert!(instance.constraints().contains_key(&ConstraintID::from(id)));
    }
}

#[test]
fn disjoint_one_hot_and_fresh_selector_sos1_effects_apply_together() {
    let one_hot_hint = one_hot_hint(50, vec![11, 12]);
    let mut sos1_hint = sos1_hint(102, vec![0, 1]);
    sos1_hint.big_m_constraint_ids = vec![100, 101];
    let bytes = raw_instance_from(
        fresh_selector_source_instance((11, 12)),
        vec![one_hot_hint],
        vec![sos1_hint],
    )
    .encode_to_vec();

    let (instance, report) =
        Instance::from_v1_bytes_with_promotion(&bytes, ATol::default()).unwrap();

    assert!(!report.has_rejections());
    assert!(report.one_hot_outcomes()[0].is_promoted());
    let sos1_promotion = report.sos1_outcomes()[0].promotion().unwrap();
    assert_eq!(
        sos1_promotion.fresh_selectors(),
        &BTreeMap::from([(VariableID::from(1), VariableID::from(10))])
    );
    assert!(instance
        .decision_variable_dependency()
        .get(&VariableID::from(10))
        .is_some());
    assert_eq!(instance.one_hot_constraints().len(), 1);
    assert_eq!(instance.sos1_constraints().len(), 1);
    assert!(instance.constraints().is_empty());
    for id in [50, 100, 101, 102] {
        assert!(instance
            .removed_constraints()
            .contains_key(&ConstraintID::from(id)));
    }
}

#[test]
fn sos1_outcomes_remain_aligned_across_conversion_rejections() {
    let first = sos1_hint(20, vec![0, 1]);
    let invalid = sos1_hint(21, vec![]);
    let last = sos1_hint(22, vec![4, 5]);
    let bytes = raw_instance_from(
        three_sos1_source_instance(),
        vec![],
        vec![first.clone(), invalid.clone(), last.clone()],
    )
    .encode_to_vec();

    let (instance, report) =
        Instance::from_v1_bytes_with_promotion(&bytes, ATol::default()).unwrap();

    let outcomes = report.sos1_outcomes();
    assert_eq!(outcomes.len(), 3);
    for (index, (outcome, hint)) in outcomes.iter().zip([&first, &invalid, &last]).enumerate() {
        assert_eq!(outcome.index(), index);
        assert_eq!(outcome.hint(), hint);
    }
    let first_promotion = outcomes[0].promotion().unwrap();
    assert!(!outcomes[1].is_promoted());
    let last_promotion = outcomes[2].promotion().unwrap();
    assert_ne!(
        first_promotion.sos1_constraint_id(),
        last_promotion.sos1_constraint_id()
    );
    assert_eq!(
        first_promotion.members(),
        &[VariableID::from(0), VariableID::from(1)]
            .into_iter()
            .collect()
    );
    assert_eq!(
        last_promotion.members(),
        &[VariableID::from(4), VariableID::from(5)]
            .into_iter()
            .collect()
    );
    assert_eq!(instance.sos1_constraints().len(), 2);
    assert!(instance.constraints().contains_key(&ConstraintID::from(21)));
    assert!(!instance.constraints().contains_key(&ConstraintID::from(20)));
    assert!(!instance.constraints().contains_key(&ConstraintID::from(22)));
}

#[test]
fn conflicting_sos1_hints_are_both_rejected_without_blocking_one_hot() {
    let first = valid_sos1_hint();
    let mut second = first.clone();
    second.decision_variables.reverse();
    let bytes =
        raw_instance_with_hints(vec![valid_one_hot_hint()], vec![first, second]).encode_to_vec();

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
