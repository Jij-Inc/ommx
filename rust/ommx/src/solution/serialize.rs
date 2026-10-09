use super::*;
use crate::{v1, Message, Parse, ParseError};
use anyhow::Result;

impl Solution {
    /// Serialize this solution as an OMMX v1 protobuf payload.
    ///
    /// # Panics
    /// Panics if an extension annotation uses the reserved `org.ommx.v1.` namespace.
    /// Use [`Self::try_to_bytes`] to handle validation errors.
    pub fn to_bytes(&self) -> Vec<u8> {
        self.try_to_bytes()
            .expect("Cannot serialize invalid OMMX extension annotations")
    }

    /// Serialize this solution after validating extension annotation keys.
    pub fn try_to_bytes(&self) -> Result<Vec<u8>, ParseError> {
        crate::parse::validate_extension_annotations(&self.annotations, "ommx.v1.Solution")?;
        let v1_solution = v1::Solution::from(self.clone());
        Ok(v1_solution.encode_to_vec())
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let inner = v1::Solution::decode(bytes)?;
        Ok(Parse::parse(inner, &())?)
    }
}
