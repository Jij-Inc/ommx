//! IO-adjacent helpers for protobuf-generated `v2::*` roots.

use crate::v2::Feature;

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
