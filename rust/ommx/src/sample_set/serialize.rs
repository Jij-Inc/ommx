use super::*;
use crate::{v1, v2, Message, Parse};
use anyhow::Result;

impl SampleSet {
    pub fn to_v1_bytes(&self) -> Vec<u8> {
        let v1_sample_set = v1::SampleSet::from(self.clone());
        v1_sample_set.encode_to_vec()
    }

    pub fn to_v2_bytes(&self) -> Vec<u8> {
        let v2_sample_set = v2::SampleSet::from(self.clone());
        v2_sample_set.encode_to_vec()
    }

    pub fn from_v1_bytes(bytes: &[u8]) -> Result<Self> {
        let inner = v1::SampleSet::decode(bytes)?;
        Ok(Parse::parse(inner, &())?)
    }

    pub fn from_v2_bytes(bytes: &[u8]) -> Result<Self> {
        let inner = v2::SampleSet::decode(bytes)?;
        Ok(Parse::parse(inner, &())?)
    }
}

impl From<SampleSet> for v2::SampleSet {
    fn from(value: SampleSet) -> Self {
        let required_features = crate::v2_io::required_features(
            !value.indicator_constraints.is_empty(),
            !value.one_hot_constraints.is_empty(),
            !value.sos1_constraints.is_empty(),
            value
                .decision_variables
                .values()
                .any(|variable| *variable.kind() == crate::Kind::FiniteDomain),
        );

        let SampleSet {
            decision_variables,
            objectives,
            constraints,
            indicator_constraints,
            one_hot_constraints,
            sos1_constraints,
            named_functions,
            sense,
            feasible,
            feasible_relaxed,
            feasibility_atol,
            metadata,
            annotations,
        } = value;

        Self {
            required_features,
            objectives: Some(objectives.into()),
            decision_variables: Some(decision_variables.into()),
            sampled_regular_constraints: Some(constraints.into()),
            feasible: sample_bool_map_to_v2(feasible),
            sense: sense.into(),
            feasible_relaxed: sample_bool_map_to_v2(feasible_relaxed),
            sampled_named_functions: Some(named_functions.into()),
            metadata,
            annotations: crate::v2_io::extension_annotations_to_v2_map(annotations),
            sampled_indicator_constraints: Some(indicator_constraints.into()),
            sampled_one_hot_constraints: Some(one_hot_constraints.into()),
            sampled_sos1_constraints: Some(sos1_constraints.into()),
            feasibility_atol: Some(feasibility_atol.into_inner()),
        }
    }
}

fn sample_bool_map_to_v2(
    map: std::collections::BTreeMap<SampleID, bool>,
) -> std::collections::BTreeMap<u64, bool> {
    map.into_iter()
        .map(|(sample_id, value)| (sample_id.into_inner(), value))
        .collect()
}
