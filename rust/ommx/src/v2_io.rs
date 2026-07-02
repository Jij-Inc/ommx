//! IO-adjacent helpers for protobuf-generated `v2::*` roots.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::v2::Feature;
use crate::{ParseError, RawParseError, SampleID, VariableID, VariableIDSet};

pub(crate) fn required_features(
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

pub(crate) fn extension_annotations_to_v2_map(
    annotations: HashMap<String, String>,
) -> BTreeMap<String, String> {
    crate::protobuf_extension_annotations(annotations)
        .into_iter()
        .collect()
}

pub(crate) fn extension_annotations_from_v2_map(
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

pub(crate) fn parse_required_features(
    features: Vec<i32>,
    message: &'static str,
) -> Result<BTreeSet<Feature>, ParseError> {
    let mut parsed = BTreeSet::new();
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
        parsed.insert(feature);
    }
    Ok(parsed)
}

pub(crate) fn validate_feature_payload(
    required_features: &BTreeSet<Feature>,
    feature: Feature,
    has_payload: bool,
    message: &'static str,
    field: &'static str,
) -> Result<(), ParseError> {
    let feature_required = required_features.contains(&feature);
    match (feature_required, has_payload) {
        (true, true) | (false, false) => Ok(()),
        (true, false) => Err(RawParseError::InvalidInstance(format!(
            "{feature:?} is listed in required_features, but {field} is empty",
        ))
        .context(message, field)),
        (false, true) => Err(RawParseError::InvalidInstance(format!(
            "{field} is non-empty, but required_features does not include {feature:?}",
        ))
        .context(message, "required_features")),
    }
}

pub(crate) fn variable_id_set_from_v2(
    ids: Vec<u64>,
    message: &'static str,
    field: &'static str,
) -> Result<VariableIDSet, ParseError> {
    let mut out = VariableIDSet::default();
    for id in ids {
        let id = VariableID::from(id);
        if !out.insert(id) {
            return Err(RawParseError::InvalidInstance(format!(
                "Duplicated variable ID is found in {field}: {id:?}",
            ))
            .context(message, field));
        }
    }
    Ok(out)
}

pub(crate) fn sample_bool_map_from_v2(map: BTreeMap<u64, bool>) -> BTreeMap<SampleID, bool> {
    map.into_iter()
        .map(|(id, value)| (SampleID::from(id), value))
        .collect()
}

pub(crate) fn sampled_active_variable_map_from_v2(
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
