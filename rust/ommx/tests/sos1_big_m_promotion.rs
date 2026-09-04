use ommx::{
    coeff, Bound, Constraint, ConstraintID, DecisionVariable, Function, Instance, Kind, Linear,
    LinearMonomial, Sense, Sos1BigMPromotionBatchPlan, Sos1BigMPromotionBatchRejected,
    Sos1BigMPromotionRequest, Sos1BigMSelectorClaim, Sos1ConstraintID, VariableID,
};
use std::collections::BTreeMap;

fn term(id: u64, coefficient: f64) -> Linear {
    Linear::single_term(
        LinearMonomial::Variable(VariableID::from(id)),
        ommx::Coefficient::try_from(coefficient).unwrap(),
    )
}

#[test]
fn public_api_promotes_a_checked_mixed_selector_formulation() {
    let upper_id = ConstraintID::from(100);
    let lower_id = ConstraintID::from(101);
    let cardinality_id = ConstraintID::from(102);
    let upper = Constraint::less_than_or_equal_to_zero(Function::from(
        (term(1, 1.0) + term(10, -3.0)).unwrap(),
    ));
    let lower = Constraint::less_than_or_equal_to_zero(Function::from(
        (term(10, -2.0) + term(1, -1.0)).unwrap(),
    ));
    let cardinality = Constraint::less_than_or_equal_to_zero(Function::from(
        ((term(0, 1.0) + term(10, 1.0)).unwrap() + Linear::from(coeff!(-1.0))).unwrap(),
    ));

    let mut instance = Instance::new(
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
        ]),
        BTreeMap::from([
            (upper_id, upper),
            (lower_id, lower),
            (cardinality_id, cardinality),
        ]),
    )
    .unwrap();
    let request = Sos1BigMPromotionRequest {
        selector_claims: BTreeMap::from([
            (VariableID::from(0), Sos1BigMSelectorClaim::Reused),
            (
                VariableID::from(1),
                Sos1BigMSelectorClaim::Fresh {
                    selector: VariableID::from(10),
                    upper_link: Some(upper_id),
                    lower_link: Some(lower_id),
                },
            ),
        ]),
        cardinality_constraint: cardinality_id,
    };
    assert_eq!(request.selector_claims.len(), 2);

    let requests = [request];
    let plan: Sos1BigMPromotionBatchPlan<'_> =
        instance.plan_promote_sos1_big_m(&requests, Default::default());
    assert!(plan.is_fully_valid());
    assert_eq!(plan.rejections().count(), 0);
    let outcomes = plan.apply();
    assert_eq!(outcomes.len(), 1);
    let promotion = outcomes
        .into_iter()
        .next()
        .expect("one request has one aligned result")
        .unwrap();

    assert_eq!(promotion.relaxed_constraint_ids().len(), 3);

    let restored = Instance::from_v2_bytes(&instance.to_v2_bytes()).unwrap();
    assert_eq!(restored, instance);
}

#[test]
fn public_api_promotes_independent_requests_with_a_shared_member() {
    let member = VariableID::from(1);
    let selectors = [VariableID::from(10), VariableID::from(11)];
    let row_ids = [
        [
            ConstraintID::from(100),
            ConstraintID::from(101),
            ConstraintID::from(102),
        ],
        [
            ConstraintID::from(200),
            ConstraintID::from(201),
            ConstraintID::from(202),
        ],
    ];
    let mut constraints = BTreeMap::new();
    let mut requests = Vec::new();
    for (selector, [upper, lower, cardinality]) in selectors.into_iter().zip(row_ids) {
        constraints.insert(
            upper,
            Constraint::less_than_or_equal_to_zero(Function::from(
                (term(member.into_inner(), 1.0) + term(selector.into_inner(), -3.0)).unwrap(),
            )),
        );
        constraints.insert(
            lower,
            Constraint::less_than_or_equal_to_zero(Function::from(
                (term(member.into_inner(), -1.0) + term(selector.into_inner(), -2.0)).unwrap(),
            )),
        );
        constraints.insert(
            cardinality,
            Constraint::less_than_or_equal_to_zero(Function::from(
                (term(selector.into_inner(), 1.0) + Linear::from(coeff!(-1.0))).unwrap(),
            )),
        );
        requests.push(Sos1BigMPromotionRequest {
            selector_claims: BTreeMap::from([(
                member,
                Sos1BigMSelectorClaim::Fresh {
                    selector,
                    upper_link: Some(upper),
                    lower_link: Some(lower),
                },
            )]),
            cardinality_constraint: cardinality,
        });
    }
    let mut instance = Instance::new(
        Sense::Minimize,
        Function::Zero,
        BTreeMap::from([
            (
                member,
                DecisionVariable::new(
                    Kind::Integer,
                    Bound::new(-2.0, 3.0).unwrap(),
                    Default::default(),
                )
                .unwrap(),
            ),
            (selectors[0], DecisionVariable::binary()),
            (selectors[1], DecisionVariable::binary()),
        ]),
        constraints,
    )
    .unwrap();

    let before = instance.clone();
    let conflicting_requests = [
        requests[0].clone(),
        requests[0].clone(),
        requests[1].clone(),
    ];
    let error = instance
        .promote_sos1_big_m_if_fully_valid(&conflicting_requests, Default::default())
        .unwrap_err();
    let rejected = error
        .downcast_ref::<Sos1BigMPromotionBatchRejected>()
        .expect("the public signal remains downcastable");
    assert_eq!(rejected.request_count(), 3);
    assert_eq!(
        rejected
            .rejections()
            .map(|(index, _)| index)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    assert_eq!(instance, before);

    let promotions = instance
        .promote_sos1_big_m_if_fully_valid(&requests, Default::default())
        .unwrap();

    assert_eq!(promotions.len(), requests.len());
    assert_eq!(
        promotions
            .iter()
            .map(|promotion| promotion.sos1_constraint_id())
            .collect::<Vec<_>>(),
        vec![Sos1ConstraintID::from(0), Sos1ConstraintID::from(1)]
    );
    assert!(instance.constraints().is_empty());
    assert_eq!(instance.removed_constraints().len(), 6);
    assert_eq!(instance.sos1_constraints().len(), 2);
}
