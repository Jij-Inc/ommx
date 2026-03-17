use super::*;
use crate::{v1, Message, Parse};
use anyhow::Result;

impl NamedFunction {
    pub fn to_bytes(&self) -> Vec<u8> {
        let v1_named_function = v1::NamedFunction::from(self.clone());
        v1_named_function.encode_to_vec()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let inner = v1::NamedFunction::decode(bytes)?;
        Ok(Parse::parse(inner, &())?)
    }
}

impl EvaluatedNamedFunction {
    pub fn to_bytes(&self) -> Vec<u8> {
        let v1_evaluated_named_function = v1::EvaluatedNamedFunction::from(self.clone());
        v1_evaluated_named_function.encode_to_vec()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let inner = v1::EvaluatedNamedFunction::decode(bytes)?;
        Ok(Parse::parse(inner, &())?)
    }
}

impl SampledNamedFunction {
    pub fn to_bytes(&self) -> Vec<u8> {
        let v1_sampled_named_function = v1::SampledNamedFunction::from(self.clone());
        v1_sampled_named_function.encode_to_vec()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let inner = v1::SampledNamedFunction::decode(bytes)?;
        Ok(Parse::parse(inner, &())?)
    }
}
