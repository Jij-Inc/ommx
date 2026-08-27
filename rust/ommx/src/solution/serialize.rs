use super::*;
use crate::{message_io, v1, v2, Message, Parse};
use anyhow::Result;

impl Solution {
    pub fn to_v1_bytes(&self) -> Vec<u8> {
        let v1_solution = v1::Solution::from(self.clone());
        v1_solution.encode_to_vec()
    }

    pub fn to_v2_bytes(&self) -> Vec<u8> {
        let v2_solution = v2::Solution::from(self.clone());
        v2_solution.encode_to_vec()
    }

    pub fn from_v1_bytes(bytes: &[u8]) -> Result<Self> {
        let inner = message_io::decode::<v1::Solution>(bytes, "ommx.v1.Solution")?;
        Ok(Parse::parse(inner, &())?)
    }

    pub fn from_v2_bytes(bytes: &[u8]) -> Result<Self> {
        let inner = message_io::decode::<v2::Solution>(bytes, "ommx.v2.Solution")?;
        Ok(Parse::parse(inner, &())?)
    }
}

impl From<Solution> for v2::Solution {
    fn from(value: Solution) -> Self {
        let required_features = crate::v2_io::required_features(
            !value.evaluated_indicator_constraints.is_empty(),
            !value.evaluated_one_hot_constraints.is_empty(),
            !value.evaluated_sos1_constraints.is_empty(),
        );
        let feasible = value.feasible();
        let feasible_relaxed = value.feasible_relaxed();

        let Solution {
            objective,
            evaluated_constraints,
            evaluated_indicator_constraints,
            evaluated_one_hot_constraints,
            evaluated_sos1_constraints,
            evaluated_named_functions,
            decision_variables,
            optimality,
            relaxation,
            sense,
            feasibility_atol,
            metadata,
            annotations,
        } = value;

        Self {
            required_features,
            objective,
            decision_variables: Some(decision_variables.into()),
            evaluated_regular_constraints: Some(evaluated_constraints.into()),
            feasible,
            optimality: optimality.into(),
            relaxation: relaxation.into(),
            feasible_relaxed: Some(feasible_relaxed),
            sense: sense.map(Into::into).unwrap_or_default(),
            evaluated_named_functions: Some(evaluated_named_functions.into()),
            metadata,
            annotations: crate::v2_io::extension_annotations_to_v2_map(annotations),
            evaluated_indicator_constraints: Some(evaluated_indicator_constraints.into()),
            evaluated_one_hot_constraints: Some(evaluated_one_hot_constraints.into()),
            evaluated_sos1_constraints: Some(evaluated_sos1_constraints.into()),
            feasibility_atol: Some(feasibility_atol.into_inner()),
        }
    }
}
