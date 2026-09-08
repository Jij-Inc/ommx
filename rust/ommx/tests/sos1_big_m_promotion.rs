use ommx::{
    coeff, Bound, Constraint, ConstraintID, DecisionVariable, Function, Instance, Kind, Linear,
    LinearMonomial, Sense, Sos1BigMPromotion, Sos1BigMPromotionBatchPlan,
    Sos1BigMPromotionBatchRejected, Sos1BigMPromotionRequest, Sos1BigMSelectorClaim,
    Sos1ConstraintID, VariableID,
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
    let request: Sos1BigMPromotionRequest = BTreeMap::from([(
        cardinality_id,
        BTreeMap::from([
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
    )]);
    assert_eq!(request[&cardinality_id].len(), 2);

    let plan: Sos1BigMPromotionBatchPlan<'_> =
        instance.plan_promote_sos1_big_m(&request, Default::default());
    assert!(plan.is_fully_valid());
    assert_eq!(plan.rejections().count(), 0);
    let outcomes: Sos1BigMPromotion = plan.apply();
    assert_eq!(outcomes.len(), 1);
    assert_eq!(
        *outcomes[&cardinality_id].as_ref().unwrap(),
        Sos1ConstraintID::from(0)
    );
    assert_eq!(instance.removed_constraints().len(), 3);

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
    let mut request = Sos1BigMPromotionRequest::new();
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
        request.insert(
            cardinality,
            BTreeMap::from([(
                member,
                Sos1BigMSelectorClaim::Fresh {
                    selector,
                    upper_link: Some(upper),
                    lower_link: Some(lower),
                },
            )]),
        );
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
    let mut invalid_request = request.clone();
    invalid_request.insert(ConstraintID::from(0), BTreeMap::new());
    invalid_request.insert(ConstraintID::from(u64::MAX), BTreeMap::new());
    let error = instance
        .promote_sos1_big_m_if_fully_valid(&invalid_request, Default::default())
        .unwrap_err();
    let rejected = error
        .downcast_ref::<Sos1BigMPromotionBatchRejected>()
        .expect("the public signal remains downcastable");
    assert_eq!(rejected.request_count(), 4);
    assert_eq!(
        rejected.rejections().map(|(id, _)| id).collect::<Vec<_>>(),
        vec![ConstraintID::from(0), ConstraintID::from(u64::MAX)]
    );
    assert_eq!(instance, before);

    let promotions: Sos1BigMPromotion = instance
        .promote_sos1_big_m_if_fully_valid(&request, Default::default())
        .unwrap();

    assert_eq!(
        promotions.keys().collect::<Vec<_>>(),
        request.keys().collect::<Vec<_>>()
    );
    assert_eq!(
        promotions
            .values()
            .map(|result| *result.as_ref().unwrap())
            .collect::<Vec<_>>(),
        vec![Sos1ConstraintID::from(0), Sos1ConstraintID::from(1)]
    );
    assert!(instance.constraints().is_empty());
    assert_eq!(instance.removed_constraints().len(), 6);
    assert_eq!(instance.sos1_constraints().len(), 2);
}
