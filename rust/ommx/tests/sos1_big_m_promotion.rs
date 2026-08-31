use ommx::{
    coeff, Bound, Constraint, ConstraintID, DecisionVariable, Function, Instance, Kind, Linear,
    LinearMonomial, Sense, Sos1BigMPromotionRequest, Sos1BigMSelectorClaim, VariableID,
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
    let request = Sos1BigMPromotionRequest::new(
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
        cardinality_id,
    );
    assert_eq!(request.selector_claims().len(), 2);

    let promotion = instance.promote_sos1_big_m(&request).unwrap();

    assert_eq!(promotion.relaxed_constraint_ids().len(), 3);

    let restored = Instance::from_v2_bytes(&instance.to_v2_bytes()).unwrap();
    assert_eq!(restored, instance);
}
