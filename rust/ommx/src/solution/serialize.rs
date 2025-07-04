use super::*;
use crate::{v1, Message, Parse};
use anyhow::Result;

impl Solution {
    pub fn to_bytes(&self) -> Vec<u8> {
        let v1_solution = v1::Solution::from(self.clone());
        v1_solution.encode_to_vec()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let inner = v1::Solution::decode(bytes)?;
        Ok(Parse::parse(inner, &())?)
    }
}