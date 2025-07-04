use super::*;
use crate::{v1, Message, Parse};
use anyhow::Result;

impl Instance {
    pub fn to_bytes(&self) -> Vec<u8> {
        let v1_instance = v1::Instance::from(self.clone());
        v1_instance.encode_to_vec()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let inner = v1::Instance::decode(bytes)?;
        Ok(Parse::parse(inner.clone(), &())?)
    }
}
