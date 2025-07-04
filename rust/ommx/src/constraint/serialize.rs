use super::*;
use crate::{v1, Message, Parse};
use anyhow::Result;

impl Constraint {
    pub fn to_bytes(&self) -> Vec<u8> {
        let v1_constraint = v1::Constraint::from(self.clone());
        v1_constraint.encode_to_vec()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let inner = v1::Constraint::decode(bytes)?;
        Ok(Parse::parse(inner, &())?)
    }
}

impl EvaluatedConstraint {
    pub fn to_bytes(&self) -> Vec<u8> {
        let v1_evaluated_constraint = v1::EvaluatedConstraint::from(self.clone());
        v1_evaluated_constraint.encode_to_vec()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let inner = v1::EvaluatedConstraint::decode(bytes)?;
        Ok(Parse::parse(inner, &())?)
    }
}

impl RemovedConstraint {
    pub fn to_bytes(&self) -> Vec<u8> {
        let v1_removed_constraint = v1::RemovedConstraint::from(self.clone());
        v1_removed_constraint.encode_to_vec()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let inner = v1::RemovedConstraint::decode(bytes)?;
        Ok(Parse::parse(inner, &())?)
    }
}

impl SampledConstraint {
    pub fn to_bytes(&self) -> Vec<u8> {
        let v1_sampled_constraint = v1::SampledConstraint::from(self.clone());
        v1_sampled_constraint.encode_to_vec()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let inner = v1::SampledConstraint::decode(bytes)?;
        Ok(Parse::parse(inner, &())?)
    }
}
