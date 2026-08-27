use super::*;
use crate::{
    parse::*,
    v1::{self, sampled_values::SampledValuesEntry, samples::SamplesEntry},
};

impl From<DuplicatedSampleIDError> for ParseError {
    fn from(e: DuplicatedSampleIDError) -> Self {
        ParseError::new(e)
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
                .map_err(ParseError::from)?;
        }
        Ok(out)
    }
}

impl TryFrom<v1::Samples> for Sampled<v1::State> {
    type Error = ParseError;
    fn try_from(value: v1::Samples) -> Result<Self, Self::Error> {
        value.parse(&())
    }
}

impl From<Sampled<v1::State>> for v1::Samples {
    fn from(sampled: Sampled<v1::State>) -> Self {
        v1::Samples {
            entries: sampled
                .chunk()
                .into_iter()
                .map(|(state, ids)| SamplesEntry {
                    state: Some(state),
                    ids: ids.into_iter().map(|id| id.into_inner()).collect(),
                })
                .collect(),
        }
    }
}

impl Parse for v1::SampledValues {
    type Output = Sampled<f64>;
    // Do not check Value against Instance about variable ID and bound.
    type Context = ();
    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let mut out = Sampled::default();
        for SampledValuesEntry { value, ids } in self.entries {
            out.append(ids.into_iter().map(SampleID::from), value)
                .map_err(ParseError::from)?;
        }
        Ok(out)
    }
}

impl TryFrom<v1::SampledValues> for Sampled<f64> {
    type Error = ParseError;
    fn try_from(value: v1::SampledValues) -> Result<Self, Self::Error> {
        value.parse(&())
    }
}

impl From<Sampled<f64>> for v1::SampledValues {
    fn from(sampled: Sampled<f64>) -> Self {
        v1::SampledValues {
            entries: sampled
                .chunk()
                .into_iter()
                .map(|(value, ids)| SampledValuesEntry {
                    value,
                    ids: ids.into_iter().map(|id| id.into_inner()).collect(),
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error as _;

    #[test]
    fn parse_preserves_duplicated_sample_id_error() {
        let error = v1::SampledValues {
            entries: vec![SampledValuesEntry {
                value: 1.0,
                ids: vec![7, 7],
            }],
        }
        .parse(&())
        .unwrap_err();

        assert_eq!(
            error
                .source()
                .and_then(|error| error.downcast_ref::<DuplicatedSampleIDError>())
                .map(DuplicatedSampleIDError::id),
            Some(SampleID::from(7))
        );
    }
}
