//! IO-adjacent helpers for protobuf-generated `v2::*` roots.
//!
//! `v2_io` itself is a private crate-root module. Its items are `pub` only so
//! sibling domain owner modules can share the same protobuf-boundary policies
//! without exporting those helpers as Rust SDK API.

use std::collections::{BTreeMap, HashMap};

use crate::constraint_type::IDType;
use crate::v2::Feature;
use crate::{
    ATol, ModelingLabelStore, ParseError, RawParseError, SampleID, Sampled, Sense, VariableID,
    VariableIDSet,
};

pub fn required_features(
    has_indicator_constraints: bool,
    has_one_hot_constraints: bool,
    has_sos1_constraints: bool,
) -> Vec<i32> {
    let mut features = Vec::new();
    if has_indicator_constraints {
        features.push(Feature::ConstraintIndicator as i32);
    }
    if has_one_hot_constraints {
        features.push(Feature::ConstraintOneHot as i32);
    }
    if has_sos1_constraints {
        features.push(Feature::ConstraintSos1 as i32);
    }
    features
}

pub fn extension_annotations_to_v2_map(
    annotations: HashMap<String, String>,
) -> BTreeMap<String, String> {
    crate::protobuf_extension_annotations(annotations)
        .into_iter()
        .collect()
}

pub fn extension_annotations_from_v2_map(
    annotations: BTreeMap<String, String>,
    message: &'static str,
) -> Result<HashMap<String, String>, ParseError> {
    for key in annotations.keys() {
        if crate::is_reserved_annotation_key(key) {
            return Err(RawParseError::ReservedAnnotationKey { key: key.clone() }
                .context(message, "annotations"));
        }
    }
    Ok(annotations.into_iter().collect())
}

/// Validate that every required feature is known to this SDK.
///
/// Payload fields are deliberately not cross-checked against this list. The
/// writer owns declaring the features its payload uses; the reader uses only
/// the declarations to decide whether it can interpret the payload.
pub fn validate_required_features(
    features: Vec<i32>,
    message: &'static str,
) -> Result<(), ParseError> {
    for value in features {
        let feature = Feature::try_from(value).map_err(|_| {
            RawParseError::UnknownEnumValue {
                enum_name: "ommx.v2.Feature",
                value,
            }
            .context(message, "required_features")
        })?;
        if feature == Feature::Unspecified {
            return Err(RawParseError::UnknownEnumValue {
                enum_name: "ommx.v2.Feature",
                value,
            }
            .context(message, "required_features"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod required_features_tests {
    use super::*;

    #[test]
    fn accepts_known_features() {
        validate_required_features(
            vec![
                Feature::ConstraintIndicator as i32,
                Feature::ConstraintOneHot as i32,
                Feature::ConstraintSos1 as i32,
                Feature::OutputObjective as i32,
            ],
            "test.Message",
        )
        .unwrap();
    }

    #[test]
    fn rejects_unspecified_and_unknown_features() {
        for feature in [Feature::Unspecified as i32, i32::MAX] {
            assert!(validate_required_features(vec![feature], "test.Message").is_err());
        }
    }

    #[test]
    fn parse_feasibility_atol_preserves_atol_signals() {
        for value in [0.0, -1.0] {
            let err = parse_feasibility_atol(Some(value), "test.Message").unwrap_err();
            assert!(matches!(
                err.error.downcast_ref::<crate::AtolError>(),
                Some(crate::AtolError::NonPositive { value: actual }) if *actual == value
            ));
        }

        let err = parse_feasibility_atol(Some(f64::NAN), "test.Message").unwrap_err();
        assert!(matches!(
            err.error.downcast_ref::<crate::AtolError>(),
            Some(crate::AtolError::NaN)
        ));
    }

    #[test]
    fn parse_feasibility_atol_keeps_infinity_as_an_ordinary_error() {
        for value in [f64::NEG_INFINITY, f64::INFINITY] {
            let err = parse_feasibility_atol(Some(value), "test.Message").unwrap_err();
            assert!(err.error.downcast_ref::<crate::AtolError>().is_none());
            assert!(err.error.downcast_ref::<RawParseError>().is_none());
            assert!(err.to_string().contains("feasibility_atol must be finite"));
        }
    }

    #[test]
    fn non_finite_values_are_ordinary_semantic_errors() {
        let err = validate_finite_f64(f64::NAN, "test.Message", "value").unwrap_err();
        assert!(err.error.downcast_ref::<RawParseError>().is_none());

        let values = Sampled::from((SampleID::from(7), f64::INFINITY));
        let err = validate_sampled_f64_values(&values, "test.Message", "values").unwrap_err();
        assert!(err.error.downcast_ref::<RawParseError>().is_none());
        assert!(err.to_string().contains("SampleID(7)"));
    }

    #[test]
    fn duplicated_variable_references_are_ordinary_semantic_errors() {
        let err = variable_id_set_from_v2(vec![1, 1], "test.Message", "variables").unwrap_err();
        assert!(err.error.downcast_ref::<RawParseError>().is_none());
        assert!(err.to_string().contains("VariableID(1)"));
    }
}

pub fn parse_feasibility_atol(
    value: Option<f64>,
    message: &'static str,
) -> Result<ATol, ParseError> {
    let Some(value) = value else {
        return Ok(ATol::default());
    };
    if value.is_infinite() {
        return Err(ParseError::new(crate::error!(
            "feasibility_atol must be finite: value={value}",
        ))
        .context(message, "feasibility_atol"));
    }
    ATol::new(value).map_err(|error| ParseError::new(error).context(message, "feasibility_atol"))
}

pub fn validate_finite_f64(
    value: f64,
    message: &'static str,
    field: &'static str,
) -> Result<(), ParseError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(
            ParseError::new(crate::error!("{field} must be finite: value={value}",))
                .context(message, field),
        )
    }
}

pub fn parse_v2_required_sense(value: i32, message: &'static str) -> Result<Sense, ParseError> {
    let sense = crate::v1::instance::Sense::try_from(value)
        .map_err(|_| RawParseError::UnknownEnumValue {
            enum_name: "ommx.v1.Sense",
            value,
        })
        .map_err(|e| ParseError::from(e).context(message, "sense"))?;
    match sense {
        crate::v1::instance::Sense::Minimize => Ok(Sense::Minimize),
        crate::v1::instance::Sense::Maximize => Ok(Sense::Maximize),
        crate::v1::instance::Sense::Unspecified => Err(RawParseError::UnknownEnumValue {
            enum_name: "ommx.v1.Sense",
            value,
        }
        .context(message, "sense")),
    }
}

pub fn validate_sampled_f64_values(
    values: &Sampled<f64>,
    message: &'static str,
    field: &'static str,
) -> Result<(), ParseError> {
    for (sample_id, value) in values.iter() {
        if !value.is_finite() {
            return Err(ParseError::new(crate::error!(
                "{field} must be finite for sample {sample_id:?}: value={value}",
            ))
            .context(message, field));
        }
    }
    Ok(())
}

pub fn variable_id_set_from_v2(
    ids: Vec<u64>,
    message: &'static str,
    field: &'static str,
) -> Result<VariableIDSet, ParseError> {
    let mut out = VariableIDSet::default();
    for id in ids {
        let id = VariableID::from(id);
        if !out.insert(id) {
            return Err(ParseError::new(crate::error!(
                "Duplicated variable ID is found in {field}: {id:?}",
            ))
            .context(message, field));
        }
    }
    Ok(out)
}

pub fn sample_bool_map_from_v2(map: BTreeMap<u64, bool>) -> BTreeMap<SampleID, bool> {
    map.into_iter()
        .map(|(id, value)| (SampleID::from(id), value))
        .collect()
}

pub fn sampled_active_variable_map_from_v2(
    map: BTreeMap<u64, crate::v2::SampledActiveVariable>,
) -> BTreeMap<SampleID, Option<VariableID>> {
    map.into_iter()
        .map(|(sample_id, value)| {
            (
                SampleID::from(sample_id),
                value.variable_id.map(VariableID::from),
            )
        })
        .collect()
}

pub fn modeling_label_store_to_v2_map<ID: IDType>(
    store: &ModelingLabelStore<ID>,
) -> BTreeMap<u64, crate::v2::ModelingLabel> {
    store
        .ids()
        .into_iter()
        .map(|id| (id.into(), store.collect_for(id).into()))
        .collect()
}

pub fn modeling_label_store_from_v2_map<ID: IDType>(
    labels: BTreeMap<u64, crate::v2::ModelingLabel>,
) -> ModelingLabelStore<ID> {
    let mut store = ModelingLabelStore::default();
    for (id, label) in labels {
        store.insert(ID::from(id), label.into());
    }
    store
}
