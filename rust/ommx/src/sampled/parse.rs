use super::*;
use crate::{
    parse::*,
    v1::{self, samples::SamplesEntry},
};

impl From<DuplicatedSampleIDError> for RawParseError {
    fn from(e: DuplicatedSampleIDError) -> Self {
        RawParseError::DuplicatedSampleID { id: e.id }
    }
}

impl From<DuplicatedSampleIDError> for ParseError {
    fn from(e: DuplicatedSampleIDError) -> Self {
        ParseError::from(RawParseError::from(e))
    }
}

impl Parse for v1::Samples {
    type Output = Sampled<v1::State>;
    // Do not check State against Instance about variable ID and bound.
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let mut out = Sampled::default();
        for SamplesEntry { state, ids } in self.entries {
            let state = state.ok_or_else(|| {
                ParseError::from(RawParseError::MissingField {
                    message: "ommx.v1.samples.SamplesEntry",
                    field: "state",
                })
            })?;
            out.append(ids.into_iter().map(SampleID::from), state)
                .map_err(|e| ParseError::from(e))?;
        }
        Ok(out)
    }
}
