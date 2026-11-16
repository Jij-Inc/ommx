use super::*;

#[test]
fn evaluate_collection() {
    let mut active = BTreeMap::new();
    active.insert(
        ConstraintID::from(1),
        Constraint::less_than_or_equal_to_zero(Function::from(linear!(1) + coeff!(-1.0))),
    );
    active.insert(
        ConstraintID::from(2),
        Constraint::equal_to_zero(Function::from(linear!(1) + coeff!(-2.0))),
    );

    let collection = ConstraintCollection::<Constraint>::new(active, BTreeMap::new()).unwrap();

    let state = v1::State {
        entries: [(1, 1.5)].into_iter().collect(),
    };
    let results = collection.evaluate(&state, ATol::default()).unwrap();

    assert_eq!(results.len(), 2);
    assert!(!results[&ConstraintID::from(1)].is_feasible(crate::ATol::default()));
    assert!(!results[&ConstraintID::from(2)].is_feasible(crate::ATol::default()));
    assert!(results.inactive_metadata().is_empty());
}

fn assert_collection_required_ids<T: ConstraintType>(make_row: impl Fn(u64) -> T::Created) {
    // Every row shares variable 0; variables 2 and 3 occur only in inactive
    // rows. Skipping feasibility never skips evaluation of those rows.
    let collection = ConstraintCollection::<T>::new(
        BTreeMap::from([(T::ID::from(1), make_row(1))]),
        BTreeMap::from([
            (T::ID::from(2), (make_row(2), inactive_metadata())),
            (
                T::ID::from(3),
                (
                    make_row(3),
                    InactiveConstraintMetadata {
                        skip_feasibility_check: true,
                        ..inactive_metadata()
                    },
                ),
            ),
        ]),
    )
    .unwrap();
    assert_eq!(
        collection.active_decision_variable_ids(),
        VariableIDSet::from([0.into(), 1.into()]),
    );
    let required = Evaluate::required_ids(&collection);
    assert_eq!(required, (0..=3).map(crate::VariableID::from).collect());

    let state = v1::State::from_iter(required.iter().map(|id| {
        let value = if id.into_inner() == 0 { 0.0 } else { 1.0 };
        (id.into_inner(), value)
    }));
    let atol = ATol::default();
    assert_eq!(collection.evaluate(&state, atol).unwrap().len(), 3);
    let samples = crate::Sampled::from((SampleID::from(7), state.clone()));
    assert_eq!(
        collection.evaluate_samples(&samples, atol).unwrap().len(),
        3
    );

    for missing_id in [2, 3] {
        let mut incomplete = state.clone();
        incomplete.entries.remove(&missing_id);
        assert!(collection.evaluate(&incomplete, atol).is_err());
        let samples = crate::Sampled::new_no_dedup(
            [
                (SampleID::from(7), state.clone()),
                (SampleID::from(9), incomplete),
            ]
            .into_iter(),
        );
        assert!(collection.evaluate_samples(&samples, atol).is_err());
    }
}

#[test]
fn regular_collection_required_ids_include_checked_and_skipped_rows() {
    assert_collection_required_ids::<Constraint>(|id| {
        Constraint::equal_to_zero(Function::from(linear!(0) + linear!(id)))
    });
}

#[test]
fn indicator_collection_required_ids_include_checked_and_skipped_rows() {
    assert_collection_required_ids::<crate::IndicatorConstraint>(|id| {
        crate::IndicatorConstraint::new(
            0.into(),
            Equality::EqualToZero,
            Function::from(linear!(id)),
            crate::IndicatorTriggerValue::One,
        )
    });
}

#[test]
fn one_hot_collection_required_ids_include_checked_and_skipped_rows() {
    assert_collection_required_ids::<crate::OneHotConstraint>(|id| {
        crate::OneHotConstraint::new(BTreeSet::from([0.into(), id.into()])).unwrap()
    });
}

#[test]
fn sos1_collection_required_ids_include_checked_and_skipped_rows() {
    assert_collection_required_ids::<crate::Sos1Constraint>(|id| {
        crate::Sos1Constraint::new(BTreeSet::from([0.into(), id.into()])).unwrap()
    });
}
