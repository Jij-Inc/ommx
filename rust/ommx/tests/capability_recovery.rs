use ommx::{
    v1, AdditionalCapability, Capabilities, DecisionVariable, Function, Instance,
    LoweredConstraintID, OneHotConstraint, OneHotConstraintID, Sense, VariableID,
};
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn public_capability_recovery_surface_is_usable_outside_the_crate() {
    let one_hot_id = OneHotConstraintID::from(4);
    let mut instance = Instance::builder()
        .sense(Sense::Minimize)
        .objective(Function::zero())
        .decision_variables(BTreeMap::from([
            (VariableID::from(0), DecisionVariable::binary()),
            (VariableID::from(1), DecisionVariable::binary()),
        ]))
        .constraints(BTreeMap::new())
        .one_hot_constraints(BTreeMap::from([(
            one_hot_id,
            OneHotConstraint::new(BTreeSet::from([VariableID::from(0), VariableID::from(1)]))
                .unwrap(),
        )]))
        .build()
        .unwrap();

    let reduction = instance
        .reduce_capabilities_with_recovery(&Capabilities::new())
        .unwrap();
    assert_eq!(
        reduction.lowered_constraints(),
        &[LoweredConstraintID::OneHot(one_hot_id)]
    );

    let requested = Capabilities::from([AdditionalCapability::OneHot]);
    let recovery = instance.recover_capability_reduction(reduction).unwrap();
    assert_eq!(
        recovery.recovered(),
        &[LoweredConstraintID::OneHot(one_hot_id)]
    );
    assert!(recovery.skipped().is_empty());
    assert_eq!(instance.required_capabilities(), requested);

    let state = v1::State::from_iter([(0, 1.0), (1, 0.0)]);
    assert_eq!(recovery.project_state(&state).unwrap(), state);
    assert_eq!(recovery.lift_state(&state).unwrap(), state);
}
