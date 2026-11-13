use super::*;

fn parse_error_source(error: &ParseError) -> &(dyn std::error::Error + 'static) {
    std::error::Error::source(error).expect("ParseError should expose its cause")
}

fn assert_ordinary_collection_sidecar_error(
    error: &ParseError,
    expected_message: &'static str,
    expected_field: &'static str,
) {
    let source = parse_error_source(error);
    assert!(source.downcast_ref::<crate::RawParseError>().is_none());
    assert!(source.downcast_ref::<crate::SolutionError>().is_none());
    assert!(source.downcast_ref::<crate::SampleSetError>().is_none());
    assert!(source.to_string().contains("unknown constraint ID"));

    assert_eq!(error.context.len(), 1);
    assert_eq!(error.context[0].message, expected_message);
    assert_eq!(error.context[0].field, expected_field);
    assert!(error
        .to_string()
        .contains(&format!("{expected_message}[{expected_field}]")));
}

#[test]
fn parse_v2_created_collection_orphan_context_reports_contexts_field() {
    let err = crate::v2::RegularConstraintCollection {
        contexts: [(
            1,
            crate::v2::ConstraintContext {
                label: Some(crate::v2::ModelingLabel {
                    name: Some("orphan".to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )]
        .into_iter()
        .collect(),
        ..Default::default()
    }
    .parse(&())
    .unwrap_err();

    assert_ordinary_collection_sidecar_error(
        &err,
        "crate::v2::RegularConstraintCollection",
        "contexts",
    );
}

#[test]
fn parse_v2_evaluated_collection_orphan_context_reports_contexts_field() {
    let atol = ATol::default();
    let err = crate::v2::EvaluatedRegularConstraintCollection {
        contexts: [(
            1,
            crate::v2::ConstraintContext {
                label: Some(crate::v2::ModelingLabel {
                    name: Some("orphan".to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )]
        .into_iter()
        .collect(),
        ..Default::default()
    }
    .parse(&atol)
    .unwrap_err();

    assert_ordinary_collection_sidecar_error(
        &err,
        "crate::v2::EvaluatedRegularConstraintCollection",
        "contexts",
    );
}

#[test]
fn parse_v2_sampled_collection_orphan_context_reports_contexts_field() {
    let atol = ATol::default();
    let err = crate::v2::SampledRegularConstraintCollection {
        contexts: [(
            1,
            crate::v2::ConstraintContext {
                label: Some(crate::v2::ModelingLabel {
                    name: Some("orphan".to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )]
        .into_iter()
        .collect(),
        ..Default::default()
    }
    .parse(&atol)
    .unwrap_err();

    assert_ordinary_collection_sidecar_error(
        &err,
        "crate::v2::SampledRegularConstraintCollection",
        "contexts",
    );
}

#[test]
fn parse_v2_evaluated_collection_reports_invalid_inactive_metadata_as_ordinary_error() {
    let err = crate::v2::EvaluatedRegularConstraintCollection {
        inactive_metadata: BTreeMap::from([(
            1,
            crate::v2::InactiveConstraintMetadata {
                reason: "test".to_string(),
                parameters: Default::default(),
                skip_feasibility_check: false,
            },
        )]),
        ..Default::default()
    }
    .parse(&ATol::default())
    .unwrap_err();

    assert_ordinary_collection_sidecar_error(
        &err,
        "crate::v2::EvaluatedRegularConstraintCollection",
        "inactive_metadata",
    );
}

#[test]
fn parse_v2_sampled_collection_reports_invalid_inactive_metadata_as_ordinary_error() {
    let err = crate::v2::SampledRegularConstraintCollection {
        inactive_metadata: BTreeMap::from([(
            1,
            crate::v2::InactiveConstraintMetadata {
                reason: "test".to_string(),
                parameters: Default::default(),
                skip_feasibility_check: false,
            },
        )]),
        ..Default::default()
    }
    .parse(&ATol::default())
    .unwrap_err();

    assert_ordinary_collection_sidecar_error(
        &err,
        "crate::v2::SampledRegularConstraintCollection",
        "inactive_metadata",
    );
}
