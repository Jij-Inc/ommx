use super::*;
use crate::Parse;
use crate::{v2, ATol, Equality};
use proptest::prelude::*;
use prost::Message;
use std::collections::BTreeSet;

fn mixed() -> Instance {
    Instance::builder()
        .sense(Sense::Minimize)
        .objective(Function::Zero)
        .decision_variables(BTreeMap::from([(0.into(), DecisionVariable::binary())]))
        .constraints(BTreeMap::from([(
            0.into(),
            Constraint::equal_to_zero(Function::Zero),
        )]))
        .indicator_constraints(BTreeMap::from([(
            1.into(),
            IndicatorConstraint::new(0.into(), Equality::EqualToZero, Function::Zero),
        )]))
        .one_hot_constraints(BTreeMap::from([(
            2.into(),
            OneHotConstraint::new(BTreeSet::from([0.into()])).unwrap(),
        )]))
        .sos1_constraints(BTreeMap::from([(
            3.into(),
            Sos1Constraint::new(BTreeSet::from([0.into()])).unwrap(),
        )]))
        .build()
        .unwrap()
}

fn change_wire_id(wire: &mut v2::Instance, family: usize, number: u64, removed: bool) {
    macro_rules! change {
        ($field:ident) => {{
            let collection = wire.$field.as_mut().unwrap();
            let old = family as u64;
            let row = collection.active.remove(&old).unwrap();
            if removed {
                collection.removed.insert(number, row);
                collection.removed_reasons.insert(
                    number,
                    v2::RemovedReason {
                        reason: "test".into(),
                        ..Default::default()
                    },
                );
            } else {
                collection.active.insert(number, row);
            }
            if let Some(context) = collection.contexts.remove(&old) {
                collection.contexts.insert(number, context);
            }
        }};
    }
    match family {
        0 => change!(regular_constraints),
        1 => change!(indicator_constraints),
        2 => change!(one_hot_constraints),
        3 => change!(sos1_constraints),
        _ => unreachable!(),
    }
}

#[test]
fn protobuf_rejects_every_cross_family_and_lifecycle_collision() {
    let families = ["regular", "indicator", "one-hot", "SOS1"];
    for first in 0..4 {
        for second in first + 1..4 {
            for first_removed in [false, true] {
                for second_removed in [false, true] {
                    let mut wire: v2::Instance = mixed().into();
                    change_wire_id(&mut wire, first, 42, first_removed);
                    change_wire_id(&mut wire, second, 42, second_removed);
                    let error = Instance::from_v2_bytes(&wire.encode_to_vec()).unwrap_err();
                    let message = format!("{error:#}");
                    assert!(message.contains("42"), "{message}");
                    for (family, removed) in [(first, first_removed), (second, second_removed)] {
                        assert!(
                            message.contains(&format!(
                                "{} {}",
                                if removed { "removed" } else { "active" },
                                families[family]
                            )),
                            "{message}"
                        );
                    }
                    let parametric = v2::ParametricInstance {
                        sense: wire.sense,
                        objective: wire.objective,
                        decision_variables: wire.decision_variables,
                        parameters: Some(Default::default()),
                        regular_constraints: wire.regular_constraints,
                        indicator_constraints: wire.indicator_constraints,
                        one_hot_constraints: wire.one_hot_constraints,
                        sos1_constraints: wire.sos1_constraints,
                        ..Default::default()
                    };
                    let error =
                        ParametricInstance::from_v2_bytes(&parametric.encode_to_vec()).unwrap_err();
                    assert!(format!("{error:#}").contains("Constraint number 42"));
                }
            }
        }
    }
}

#[test]
fn builders_reject_cross_family_ids() {
    let indicator = BTreeMap::from([(
        0.into(),
        IndicatorConstraint::new(0.into(), Equality::EqualToZero, Function::Zero),
    )]);
    let regular = BTreeMap::from([(0.into(), Constraint::equal_to_zero(Function::Zero))]);
    let variables = BTreeMap::from([(0.into(), DecisionVariable::binary())]);
    assert!(Instance::builder()
        .sense(Sense::Minimize)
        .objective(Function::Zero)
        .decision_variables(variables.clone())
        .constraints(regular.clone())
        .indicator_constraints(indicator.clone())
        .build()
        .unwrap_err()
        .to_string()
        .contains("Constraint number 0"));
    assert!(ParametricInstance::builder()
        .sense(Sense::Minimize)
        .objective(Function::Zero)
        .parameters(ParameterTable::default())
        .decision_variables(variables)
        .constraints(regular)
        .indicator_constraints(indicator)
        .build()
        .unwrap_err()
        .to_string()
        .contains("Constraint number 0"));
}

#[test]
fn additions_and_transformations_allocate_across_all_families() {
    let mut instance = mixed();
    assert_eq!(instance.next_constraint_id(), 4.into());
    let regular = instance.convert_one_hot_to_constraint(2.into()).unwrap();
    assert_eq!(regular, 4.into());
    assert!(instance
        .removed_one_hot_constraints()
        .contains_key(&2.into()));
    let promoted = instance.promote_one_hot(&BTreeSet::from([regular]));
    let one_hot = *promoted[&regular].as_ref().unwrap();
    assert_eq!(one_hot, 5.into());
    instance.validate_constraint_ids().unwrap();
    let mut parametric: ParametricInstance = instance.into();
    assert_eq!(
        parametric
            .add_sos1_constraint(
                Sos1Constraint::new(BTreeSet::from([0.into()])).unwrap(),
                Default::default()
            )
            .unwrap(),
        6.into()
    );
    assert_eq!(
        parametric
            .add_constraint(
                Constraint::equal_to_zero(Function::Zero),
                Default::default()
            )
            .unwrap(),
        7.into()
    );
    parametric.validate_constraint_ids().unwrap();
}

#[test]
fn explicit_insertions_reject_other_families_before_any_mutation() {
    let mut instance = mixed();
    instance.convert_one_hot_to_constraint(2.into()).unwrap();
    for number in [1, 2, 3] {
        let before = instance.clone();
        assert!(instance
            .insert_constraint(number.into(), Constraint::equal_to_zero(Function::Zero))
            .is_err());
        assert_eq!(instance, before);
        assert!(instance
            .insert_constraints(vec![
                (99.into(), Constraint::equal_to_zero(Function::Zero)),
                (number.into(), Constraint::equal_to_zero(Function::Zero))
            ])
            .is_err());
        assert_eq!(instance, before);
    }
    assert!(instance
        .insert_constraint(0.into(), Constraint::equal_to_zero(Function::Zero))
        .unwrap()
        .is_some());
}

#[test]
fn removed_special_maximum_exhausts_all_model_allocators_atomically() {
    let mut wire: v2::Instance = mixed().into();
    change_wire_id(&mut wire, 3, u64::MAX, true);
    let mut instance = Instance::try_from(wire).unwrap();
    let before = instance.clone();
    assert!(instance
        .add_constraint(
            Constraint::equal_to_zero(Function::Zero),
            Default::default()
        )
        .is_err());
    assert!(instance
        .add_indicator_constraint(
            IndicatorConstraint::new(0.into(), Equality::EqualToZero, Function::Zero),
            Default::default()
        )
        .is_err());
    assert!(instance
        .add_one_hot_constraint(
            OneHotConstraint::new(BTreeSet::from([0.into()])).unwrap(),
            Default::default()
        )
        .is_err());
    assert!(instance
        .add_sos1_constraint(
            Sos1Constraint::new(BTreeSet::from([0.into()])).unwrap(),
            Default::default()
        )
        .is_err());
    assert!(instance.convert_one_hot_to_constraint(2.into()).is_err());
    assert_eq!(instance, before);

    // Activating the indicator would generate a regular row. Propagation must
    // also respect numbers reserved by a removed constraint of another family.
    assert!(instance
        .partial_evaluate(&v1::State::from_iter([(0, 1.0)]), ATol::default())
        .is_err());
    assert_eq!(instance, before);
}

#[test]
fn batch_capacity_includes_removed_constraints_of_other_families() {
    let mut wire: v2::Instance = mixed().into();
    change_wire_id(&mut wire, 3, u64::MAX - 2, true);
    let mut instance = Instance::try_from(wire).unwrap();
    instance.ensure_constraint_id_capacity(2).unwrap();
    assert!(instance.ensure_constraint_id_capacity(3).is_err());
    assert_eq!(
        instance
            .add_constraint(
                Constraint::equal_to_zero(Function::Zero),
                Default::default()
            )
            .unwrap(),
        (u64::MAX - 1).into()
    );
    assert_eq!(
        instance
            .add_one_hot_constraint(
                OneHotConstraint::new(BTreeSet::from([0.into()])).unwrap(),
                Default::default()
            )
            .unwrap(),
        u64::MAX.into()
    );
    instance.ensure_constraint_id_capacity(0).unwrap();
    assert!(instance.ensure_constraint_id_capacity(1).is_err());
    instance.validate_constraint_ids().unwrap();
}

proptest! {
    #[test]
    fn mixed_transformations_preserve_unique_numbers_and_roundtrip(
        mut instance in Instance::arbitrary_with(InstanceParameters::full_v3())
    ) {
        instance.validate_constraint_ids().unwrap();
        instance.convert_all_one_hots_to_constraints().unwrap();
        instance.convert_all_sos1_to_constraints().unwrap();
        instance.convert_all_indicators_to_constraints(ATol::default()).unwrap();
        instance.validate_constraint_ids().unwrap();
        prop_assert_eq!(Instance::from_v2_bytes(&instance.to_v2_bytes()).unwrap(), instance);
    }
}

#[test]
fn result_builders_and_protobuf_reject_duplicate_numbers_including_removed() {
    for first in 0..4 {
        for second in first + 1..4 {
            for first_removed in [false, true] {
                for second_removed in [false, true] {
                    let mut wire: v2::Instance = mixed().into();
                    change_wire_id(&mut wire, first, first as u64, first_removed);
                    change_wire_id(&mut wire, second, second as u64, second_removed);
                    let instance = Instance::try_from(wire).unwrap();
                    let state = v1::State::from_iter([(0, 1.0)]);
                    let solution = instance.evaluate(&state, ATol::default()).unwrap();
                    let samples = instance
                        .evaluate_samples(&crate::Sampled::from((0.into(), state)), ATol::default())
                        .unwrap();
                    let message = format!("Constraint number {first}");

                    // Move the second family's row and sidecars onto the first
                    // family's number. Each collection is still valid alone.
                    macro_rules! collide {
                        ($collection:expr) => {{
                            let collection = $collection.as_mut().unwrap();
                            let old = second as u64;
                            let new = first as u64;
                            let row = collection.entries.remove(&old).unwrap();
                            collection.entries.insert(new, row);
                            if let Some(reason) = collection.removed_reasons.remove(&old) {
                                collection.removed_reasons.insert(new, reason);
                            }
                            if let Some(context) = collection.contexts.remove(&old) {
                                collection.contexts.insert(new, context);
                            }
                        }};
                    }
                    let mut wire: v2::Solution = solution.clone().into();
                    match second {
                        1 => collide!(wire.evaluated_indicator_constraints),
                        2 => collide!(wire.evaluated_one_hot_constraints),
                        3 => collide!(wire.evaluated_sos1_constraints),
                        _ => unreachable!(),
                    }
                    let error = crate::Solution::from_v2_bytes(&wire.encode_to_vec()).unwrap_err();
                    assert!(format!("{error:#}").contains(&message));
                    let atol = ATol::default();
                    let error = crate::Solution::builder()
                        .objective(0.0)
                        .sense(Sense::Minimize)
                        .decision_variables(solution.decision_variables().clone())
                        .evaluated_constraints_collection(
                            wire.evaluated_regular_constraints
                                .unwrap()
                                .parse(&atol)
                                .unwrap(),
                        )
                        .evaluated_indicator_constraints_collection(
                            wire.evaluated_indicator_constraints
                                .unwrap()
                                .parse(&atol)
                                .unwrap(),
                        )
                        .evaluated_one_hot_constraints_collection(
                            wire.evaluated_one_hot_constraints
                                .unwrap()
                                .parse_with_values(solution.decision_variable_table(), atol)
                                .unwrap(),
                        )
                        .evaluated_sos1_constraints_collection(
                            wire.evaluated_sos1_constraints
                                .unwrap()
                                .parse_with_values(solution.decision_variable_table(), atol)
                                .unwrap(),
                        )
                        .build()
                        .unwrap_err();
                    assert!(error.to_string().contains(&message));

                    let mut wire: v2::SampleSet = samples.clone().into();
                    match second {
                        1 => collide!(wire.sampled_indicator_constraints),
                        2 => collide!(wire.sampled_one_hot_constraints),
                        3 => collide!(wire.sampled_sos1_constraints),
                        _ => unreachable!(),
                    }
                    let error = crate::SampleSet::from_v2_bytes(&wire.encode_to_vec()).unwrap_err();
                    assert!(format!("{error:#}").contains(&message));
                    let error = crate::SampleSet::builder()
                        .sense(Sense::Minimize)
                        .objectives(samples.objectives().clone())
                        .decision_variables(samples.decision_variables().clone())
                        .constraints_collection(
                            wire.sampled_regular_constraints
                                .unwrap()
                                .parse(&atol)
                                .unwrap(),
                        )
                        .indicator_constraints_collection(
                            wire.sampled_indicator_constraints
                                .unwrap()
                                .parse(&atol)
                                .unwrap(),
                        )
                        .one_hot_constraints_collection(
                            wire.sampled_one_hot_constraints
                                .unwrap()
                                .parse_with_values(samples.decision_variable_table(), atol)
                                .unwrap(),
                        )
                        .sos1_constraints_collection(
                            wire.sampled_sos1_constraints
                                .unwrap()
                                .parse_with_values(samples.decision_variable_table(), atol)
                                .unwrap(),
                        )
                        .build()
                        .unwrap_err();
                    assert!(matches!(
                        error,
                        crate::SampleSetError::ConflictingConstraintNumber { number, .. } if number == first as u64
                    ));
                }
            }
        }
    }
}
