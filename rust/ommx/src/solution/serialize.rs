use super::*;
use crate::{v1, Message, Parse};
use anyhow::Result;

impl Solution {
    /// Serialize this solution as an OMMX v1 protobuf payload.
    ///
    /// Reserved `org.ommx.v1.*` keys in the extension map are omitted.
    /// Other keys and their string values are preserved without modification.
    pub fn to_bytes(&self) -> Vec<u8> {
        let v1_solution = v1::Solution::from(self.clone());
        v1_solution.encode_to_vec()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let inner = v1::Solution::decode(bytes)?;
        Ok(Parse::parse(inner, &())?)
    }
}
