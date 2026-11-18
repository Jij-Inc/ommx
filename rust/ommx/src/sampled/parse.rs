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
                .map_err(ParseError::new)?;
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
                .map_err(ParseError::new)?;
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
    use proptest::prelude::*;
    use std::error::Error as _;

    #[test]
    fn parsers_reject_entries_without_ids_and_preserve_the_signal() {
        let samples = v1::Samples {
            entries: vec![SamplesEntry {
                state: Some(v1::State::default()),
                ids: vec![],
            }],
        }
        .parse(&())
        .unwrap_err();
        let values = v1::SampledValues {
            entries: vec![SampledValuesEntry {
                value: 1.0,
                ids: vec![],
            }],
        }
        .parse(&())
        .unwrap_err();
        for error in [samples, values] {
            assert!(error.source().unwrap().is::<EmptySampleIDs>());
        }
    }

    #[test]
    fn empty_constants_roundtrip_without_empty_id_entries() {
        let samples: v1::Samples =
            Sampled::constants(std::iter::empty(), v1::State::default()).into();
        assert!(samples.entries.is_empty());
        assert!(samples.parse(&()).unwrap().chunk().is_empty());
        let values: v1::SampledValues = Sampled::constants(std::iter::empty(), 1.0).into();
        assert!(values.entries.is_empty());
        assert!(values.parse(&()).unwrap().chunk().is_empty());
    }

    proptest! {
        #[test]
        fn constructors_reject_duplicate_ids_or_roundtrip(
            pairs in prop::collection::vec((0u64..5, 0i32..5), 0..30)
        ) {
            let expected: FnvHashMap<_, _> = pairs.iter().copied().collect();
            let mut seen = FnvHashSet::default();
            let duplicate = pairs.iter().find(|(id, _)| !seen.insert(*id)).map(|&(id, _)| id);
            for result in [
                Sampled::new_no_dedup(pairs.iter().map(|&(id, value)| (SampleID(id), value))),
                Sampled::new_dedup(pairs.iter().map(|&(id, value)| (SampleID(id), value))),
            ] {
                if let Some(id) = duplicate {
                    let error = result.unwrap_err();
                    prop_assert_eq!(
                        error.downcast_ref::<DuplicatedSampleIDError>().unwrap().id(),
                        SampleID(id),
                    );
                    continue;
                }
                let sampled = result.unwrap();
                prop_assert_eq!(sampled.num_samples(), expected.len());
                for (&id, value) in &expected {
                    prop_assert_eq!(sampled.get(SampleID(id)), Some(value));
                }
                prop_assert!(sampled.clone().chunk().iter().all(|(_, ids)| !ids.is_empty()));

                let values = sampled.clone().map(f64::from);
                let encoded: v1::SampledValues = values.clone().into();
                prop_assert_eq!(encoded.parse(&()).unwrap(), values);

                let samples = sampled.map(|value| v1::State {
                    entries: [(0, f64::from(value))].into(),
                });
                let encoded: v1::Samples = samples.clone().into();
                prop_assert_eq!(encoded.parse(&()).unwrap(), samples);
            }
        }

        #[test]
        fn sample_groups_roundtrip(
            samples in (0usize..20).prop_flat_map(|count| crate::random::arbitrary_samples(
                crate::random::SamplesParameters::new(count.div_ceil(2), count, 100).unwrap(),
                crate::random::arbitrary_state([crate::VariableID::from(0)].into()),
            ))
        ) {
            let encoded: v1::Samples = samples.clone().into();
            prop_assert!(encoded.entries.iter().all(|entry| !entry.ids.is_empty()));
            let restored = encoded.parse(&()).unwrap();
            prop_assert_eq!(restored.clone().chunk().len(), samples.clone().chunk().len());
            prop_assert_eq!(restored, samples.clone());

            let values = samples.map(|state| state.entries[&0]);
            let encoded: v1::SampledValues = values.clone().into();
            prop_assert!(encoded.entries.iter().all(|entry| !entry.ids.is_empty()));
            let restored = encoded.parse(&()).unwrap();
            prop_assert_eq!(restored.clone().chunk().len(), values.clone().chunk().len());
            prop_assert_eq!(restored, values);
        }
    }

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
