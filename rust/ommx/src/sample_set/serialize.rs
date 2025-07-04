use super::*;
use crate::{v1, Message, Parse};
use anyhow::Result;

impl SampleSet {
    pub fn to_bytes(&self) -> Vec<u8> {
        let v1_sample_set = v1::SampleSet::from(self.clone());
        v1_sample_set.encode_to_vec()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let inner = v1::SampleSet::decode(bytes)?;
        Ok(Parse::parse(inner, &())?)
    }
}
