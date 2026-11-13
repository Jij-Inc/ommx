use super::*;

#[test]
fn empty_collection() {
    let collection = ConstraintCollection::<Constraint>::default();
    assert!(collection.active().is_empty());
    assert!(collection.inactive().is_empty());
}

#[test]
fn collection_accessors() {
    let mut active = BTreeMap::new();
    active.insert(
        ConstraintID::from(1),
        Constraint::equal_to_zero(Function::Zero),
    );

    let collection = ConstraintCollection::<Constraint>::new(active, BTreeMap::new()).unwrap();
    assert_eq!(collection.active().len(), 1);
    assert_eq!(collection.inactive().len(), 0);
}

#[test]
fn collection_rejects_active_inactive_overlap() {
    let id = ConstraintID::from(1);
    let active = BTreeMap::from([(id, Constraint::equal_to_zero(Function::Zero))]);
    let inactive = BTreeMap::from([(
        id,
        (
            Constraint::equal_to_zero(Function::Zero),
            inactive_metadata(),
        ),
    )]);

    let err = ConstraintCollection::<Constraint>::new(active, inactive).unwrap_err();
    assert!(err
        .to_string()
        .contains("appears in both active constraints and inactive constraints"));
}

#[test]
fn collection_rejects_orphan_context_id() {
    let id = ConstraintID::from(1);
    let orphan_id = ConstraintID::from(99);
    let active = BTreeMap::from([(id, Constraint::equal_to_zero(Function::Zero))]);
    let mut context = ConstraintContextStore::default();
    context.set_name(orphan_id, "orphan");

    let err = ConstraintCollection::<Constraint>::with_context(active, BTreeMap::new(), context)
        .unwrap_err();

    assert!(
        err.to_string().contains("unknown constraint ID")
            && err.to_string().contains("ConstraintID(99)"),
        "unexpected error: {err}"
    );
}

#[test]
fn set_context_for_owner_rejects_unknown_id_without_orphan() {
    let id = ConstraintID::from(1);
    let orphan_id = ConstraintID::from(99);
    let active = BTreeMap::from([(id, Constraint::equal_to_zero(Function::Zero))]);
    let mut collection = ConstraintCollection::<Constraint>::new(active, BTreeMap::new()).unwrap();

    let context = ConstraintContext {
        label: ModelingLabel {
            name: Some("orphan".to_string()),
            ..Default::default()
        },
        provenance: vec![],
    };
    let err = collection
        .set_context_for_owner(orphan_id, context, "constraint")
        .unwrap_err();

    assert!(
        err.to_string().contains("unknown constraint ID")
            && err.to_string().contains("ConstraintID(99)"),
        "unexpected error: {err}"
    );
    assert!(!collection.context().contains(orphan_id));
    collection.validate_context_ids().unwrap();
}

#[test]
fn replace_and_remove_active_rows_preserves_context() {
    let inactive_id = ConstraintID::from(1);
    let active_id = ConstraintID::from(2);
    let active = BTreeMap::from([
        (inactive_id, Constraint::equal_to_zero(Function::Zero)),
        (
            active_id,
            Constraint::equal_to_zero(Function::from(linear!(1))),
        ),
    ]);
    let mut context = ConstraintContextStore::default();
    context.set_name(inactive_id, "original");
    let mut collection =
        ConstraintCollection::<Constraint>::with_context(active, BTreeMap::new(), context).unwrap();

    let inactive = collection.active().get(&inactive_id).unwrap().clone();
    collection
        .replace_and_remove_active_rows(
            BTreeMap::new(),
            BTreeMap::from([(inactive_id, (inactive, inactive_metadata()))]),
        )
        .unwrap();

    assert!(!collection.active().contains_key(&inactive_id));
    assert!(collection.active().contains_key(&active_id));
    assert!(collection.inactive().contains_key(&inactive_id));
    assert_eq!(collection.context().name(inactive_id), Some("original"));
    collection.validate_context_ids().unwrap();
}

#[test]
fn rewrite_active_rows_by_value_preserves_context_and_lifecycle() {
    let inactive_id = ConstraintID::from(1);
    let active_id = ConstraintID::from(2);
    let previously_inactive_id = ConstraintID::from(3);
    let active = BTreeMap::from([
        (
            inactive_id,
            Constraint::equal_to_zero(Function::from(linear!(1))),
        ),
        (
            active_id,
            Constraint::equal_to_zero(Function::from(linear!(2))),
        ),
    ]);
    let inactive = BTreeMap::from([(
        previously_inactive_id,
        (
            Constraint::equal_to_zero(Function::from(linear!(3))),
            inactive_metadata(),
        ),
    )]);
    let mut context = ConstraintContextStore::default();
    context.set_name(inactive_id, "moved");
    context.set_name(active_id, "active");
    context.set_name(previously_inactive_id, "previously inactive");
    let collection =
        ConstraintCollection::<Constraint>::with_context(active, inactive, context).unwrap();

    let collection = collection
        .rewrite_active_rows_by_value(|id, row| {
            Ok(if id == inactive_id {
                ActiveRowRewrite::Inactive(row, inactive_metadata())
            } else {
                ActiveRowRewrite::Active(row)
            })
        })
        .unwrap();

    assert!(!collection.active().contains_key(&inactive_id));
    assert!(collection.active().contains_key(&active_id));
    assert!(collection.inactive().contains_key(&inactive_id));
    assert!(collection.inactive().contains_key(&previously_inactive_id));
    assert_eq!(collection.context().name(inactive_id), Some("moved"));
    assert_eq!(collection.context().name(active_id), Some("active"));
    assert_eq!(
        collection.context().name(previously_inactive_id),
        Some("previously inactive")
    );
    collection.validate_context_ids().unwrap();
}

#[test]
fn rewrite_active_rows_by_value_propagates_error_without_partial_value() {
    let id = ConstraintID::from(1);
    let collection = ConstraintCollection::<Constraint>::new(
        BTreeMap::from([(id, Constraint::equal_to_zero(Function::from(linear!(1))))]),
        BTreeMap::new(),
    )
    .unwrap();

    let error = collection
        .rewrite_active_rows_by_value(|_id, _row| Err(crate::error!("owned row rewrite failed")))
        .unwrap_err();

    assert_eq!(error.to_string(), "owned row rewrite failed");
}

#[test]
fn move_active_rows_with_reasons_rejects_unknown_id_before_mutation() {
    let active_id = ConstraintID::from(1);
    let unknown_id = ConstraintID::from(99);
    let mut collection = ConstraintCollection::<Constraint>::new(
        BTreeMap::from([(
            active_id,
            Constraint::equal_to_zero(Function::from(linear!(1)).abs()),
        )]),
        BTreeMap::new(),
    )
    .unwrap();
    let original = collection.clone();

    let error = collection
        .move_active_rows_to_inactive_with_reasons(BTreeMap::from([
            (active_id, inactive_metadata()),
            (unknown_id, inactive_metadata()),
        ]))
        .unwrap_err();

    assert!(error.to_string().contains("not found"));
    assert_eq!(collection, original);
}

#[test]
fn move_active_rows_with_reasons_moves_payload_and_preserves_context() {
    let inactive_id = ConstraintID::from(1);
    let active_id = ConstraintID::from(2);
    let active = BTreeMap::from([
        (
            inactive_id,
            Constraint::equal_to_zero(Function::from(linear!(1)).abs()),
        ),
        (
            active_id,
            Constraint::equal_to_zero(Function::from(linear!(1))),
        ),
    ]);
    let mut context = ConstraintContextStore::default();
    context.set_name(inactive_id, "original");
    let mut collection =
        ConstraintCollection::<Constraint>::with_context(active, BTreeMap::new(), context).unwrap();
    let Function::Expression(expression) =
        collection.active().get(&inactive_id).unwrap().function()
    else {
        panic!("absolute value must use expression storage")
    };
    let original_instructions = crate::function::operation::instructions(expression).as_ptr();

    collection
        .move_active_rows_to_inactive_with_reasons(BTreeMap::from([(
            inactive_id,
            inactive_metadata(),
        )]))
        .unwrap();

    let Function::Expression(expression) = collection
        .inactive()
        .get(&inactive_id)
        .unwrap()
        .0
        .function()
    else {
        panic!("moved constraint must preserve expression storage")
    };
    assert_eq!(
        crate::function::operation::instructions(expression).as_ptr(),
        original_instructions
    );
    assert!(collection.active().contains_key(&active_id));
    assert_eq!(collection.context().name(inactive_id), Some("original"));
    collection.validate_context_ids().unwrap();
}

#[test]
fn replace_active_rows_rejects_unknown_id_before_mutation() {
    let id = ConstraintID::from(1);
    let orphan_id = ConstraintID::from(99);
    let original = Constraint::equal_to_zero(Function::Zero);
    let mut collection = ConstraintCollection::<Constraint>::new(
        BTreeMap::from([(id, original.clone())]),
        BTreeMap::new(),
    )
    .unwrap();

    let err = collection
        .replace_active_rows(BTreeMap::from([
            (id, Constraint::less_than_or_equal_to_zero(Function::Zero)),
            (orphan_id, Constraint::equal_to_zero(Function::Zero)),
        ]))
        .unwrap_err();

    assert!(
        err.to_string().contains("Active constraint with ID")
            && err.to_string().contains("ConstraintID(99)"),
        "unexpected error: {err}"
    );
    assert_eq!(collection.active().get(&id), Some(&original));
    assert!(collection.inactive().is_empty());
    collection.validate_context_ids().unwrap();
}

#[test]
fn replace_rows_preserving_lifecycle_keeps_reasons_and_context() {
    let active_id = ConstraintID::from(1);
    let inactive_id = ConstraintID::from(2);
    let reason = inactive_metadata();
    let mut context = ConstraintContextStore::default();
    context.set_name(active_id, "active");
    context.set_name(inactive_id, "inactive");
    let mut collection = ConstraintCollection::<Constraint>::with_context(
        BTreeMap::from([(active_id, Constraint::equal_to_zero(Function::Zero))]),
        BTreeMap::from([(
            inactive_id,
            (
                Constraint::less_than_or_equal_to_zero(Function::Zero),
                reason.clone(),
            ),
        )]),
        context,
    )
    .unwrap();

    collection
        .replace_rows_preserving_lifecycle(
            BTreeMap::from([(
                active_id,
                Constraint::less_than_or_equal_to_zero(Function::Zero),
            )]),
            BTreeMap::from([(inactive_id, Constraint::equal_to_zero(Function::Zero))]),
        )
        .unwrap();

    assert_eq!(
        collection.active().get(&active_id).map(|c| c.equality),
        Some(Equality::LessThanOrEqualToZero)
    );
    assert_eq!(
        collection
            .inactive()
            .get(&inactive_id)
            .map(|(c, _)| c.equality),
        Some(Equality::EqualToZero)
    );
    assert_eq!(
        collection.inactive().get(&inactive_id).map(|(_, r)| r),
        Some(&reason)
    );
    assert_eq!(collection.context().name(active_id), Some("active"));
    assert_eq!(collection.context().name(inactive_id), Some("inactive"));
    collection.validate_context_ids().unwrap();
}

#[test]
fn replace_rows_preserving_lifecycle_rejects_unknown_inactive_id_before_mutation() {
    let active_id = ConstraintID::from(1);
    let inactive_id = ConstraintID::from(2);
    let orphan_id = ConstraintID::from(99);
    let original_active = Constraint::equal_to_zero(Function::Zero);
    let original_inactive = Constraint::less_than_or_equal_to_zero(Function::Zero);
    let reason = inactive_metadata();
    let mut collection = ConstraintCollection::<Constraint>::new(
        BTreeMap::from([(active_id, original_active.clone())]),
        BTreeMap::from([(inactive_id, (original_inactive.clone(), reason.clone()))]),
    )
    .unwrap();

    let err = collection
        .replace_rows_preserving_lifecycle(
            BTreeMap::from([(
                active_id,
                Constraint::less_than_or_equal_to_zero(Function::Zero),
            )]),
            BTreeMap::from([(orphan_id, Constraint::equal_to_zero(Function::Zero))]),
        )
        .unwrap_err();

    assert!(
        err.to_string().contains("Inactive constraint with ID")
            && err.to_string().contains("ConstraintID(99)"),
        "unexpected error: {err}"
    );
    assert_eq!(collection.active().get(&active_id), Some(&original_active));
    assert_eq!(
        collection.inactive().get(&inactive_id),
        Some(&(original_inactive, reason))
    );
    collection.validate_context_ids().unwrap();
}

#[test]
fn activate_row_preserves_context() {
    let id = ConstraintID::from(1);
    let inactive = Constraint::less_than_or_equal_to_zero(Function::Zero);
    let mut context = ConstraintContextStore::default();
    context.set_name(id, "activated");
    let mut collection = ConstraintCollection::<Constraint>::with_context(
        BTreeMap::new(),
        BTreeMap::from([(id, (inactive.clone(), inactive_metadata()))]),
        context,
    )
    .unwrap();

    let mut activated = inactive.clone();
    activated.equality = Equality::EqualToZero;
    let reason = collection.activate_row(id, activated).unwrap();
    assert_eq!(reason.reason, "test");
    assert!(collection
        .active()
        .get(&id)
        .is_some_and(|c| c.equality == Equality::EqualToZero));
    assert!(!collection.inactive().contains_key(&id));
    assert_eq!(collection.context().name(id), Some("activated"));
    collection.validate_context_ids().unwrap();
}

#[test]
fn insert_active_with_context_rejects_duplicate_ids() {
    let id = ConstraintID::from(1);
    let mut collection = ConstraintCollection::<Constraint>::new(
        BTreeMap::from([(id, Constraint::equal_to_zero(Function::Zero))]),
        BTreeMap::new(),
    )
    .unwrap();

    let err = collection
        .insert_active_with_context(
            id,
            Constraint::equal_to_zero(Function::Zero),
            ConstraintContext::default(),
        )
        .unwrap_err();
    assert!(err.to_string().contains("already exists in the active"));

    let inactive_id = ConstraintID::from(2);
    let mut collection = ConstraintCollection::<Constraint>::new(
        BTreeMap::new(),
        BTreeMap::from([(
            inactive_id,
            (
                Constraint::equal_to_zero(Function::Zero),
                inactive_metadata(),
            ),
        )]),
    )
    .unwrap();

    let err = collection
        .insert_active_with_context(
            inactive_id,
            Constraint::equal_to_zero(Function::Zero),
            ConstraintContext::default(),
        )
        .unwrap_err();
    assert!(err.to_string().contains("already exists in the inactive"));
}

#[test]
fn evaluated_collection_rejects_inactive_metadata_without_constraint() {
    let constraints: BTreeMap<ConstraintID, EvaluatedConstraint> = BTreeMap::new();
    let inactive_metadata = BTreeMap::from([(ConstraintID::from(1), inactive_metadata())]);

    let err = EvaluatedCollection::<Constraint>::new(constraints, inactive_metadata).unwrap_err();
    assert!(err
        .to_string()
        .contains("Inactive metadata references unknown constraint ID"));
}

#[test]
fn sampled_collection_rejects_inactive_metadata_without_constraint() {
    let constraints: BTreeMap<ConstraintID, SampledConstraint> = BTreeMap::new();
    let inactive_metadata = BTreeMap::from([(ConstraintID::from(1), inactive_metadata())]);

    let err = SampledCollection::<Constraint>::new(constraints, inactive_metadata).unwrap_err();
    assert!(err
        .to_string()
        .contains("Inactive metadata references unknown constraint ID"));
}
