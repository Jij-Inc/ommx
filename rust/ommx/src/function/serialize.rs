use super::*;
use crate::{v1, Message, Parse};
use anyhow::Result;

impl Function {
    pub fn to_bytes(&self) -> Vec<u8> {
        let v1_function = v1::Function::from(self.clone());
        v1_function.encode_to_vec()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let inner = v1::Function::decode(bytes)?;
        Ok(Parse::parse(inner, &())?)
    }
}