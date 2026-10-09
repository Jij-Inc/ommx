use super::*;
use crate::{v1, Message, Parse, ParseError};
use anyhow::Result;

impl SampleSet {
    /// Serialize this sample set as an OMMX v1 protobuf payload.
    ///
    /// # Panics
    /// Panics if an extension annotation uses the reserved `org.ommx.v1.` namespace.
    /// Use [`Self::try_to_bytes`] to handle validation errors.
    pub fn to_bytes(&self) -> Vec<u8> {
        self.try_to_bytes()
            .expect("Cannot serialize invalid OMMX extension annotations")
    }

    /// Serialize this sample set after validating extension annotation keys.
    pub fn try_to_bytes(&self) -> Result<Vec<u8>, ParseError> {
        crate::parse::validate_extension_annotations(&self.annotations, "ommx.v1.SampleSet")?;
        let v1_sample_set = v1::SampleSet::from(self.clone());
        Ok(v1_sample_set.encode_to_vec())
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let inner = v1::SampleSet::decode(bytes)?;
        Ok(Parse::parse(inner, &())?)
    }
}
