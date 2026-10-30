use crate::{
    random::{arbitrary_integer_partition, unique_integers},
    v1::State,
    SampleID, Sampled,
};
use getset::Getters;
use proptest::prelude::*;

/// Invalid parameter combinations for random sample generation.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SamplesParametersError {
    /// More distinct states were requested than total samples.
    #[error(
        "num_different_samples({num_different_samples}) must be less than or equal to num_samples({num_samples})"
    )]
    TooManyDifferentSamples {
        num_different_samples: usize,
        num_samples: usize,
    },

    /// Positive samples cannot be partitioned into zero distinct states.
    #[error("num_different_samples must be positive when num_samples is {num_samples}")]
    ZeroDifferentSamples { num_samples: usize },

    /// The inclusive sample-ID range cannot provide enough unique IDs.
    #[error(
        "num_samples({num_samples}) exceeds the sample ID capacity for max_sample_id({max_sample_id})"
    )]
    InsufficientSampleIdSpace {
        num_samples: usize,
        max_sample_id: u64,
    },
}

/// Validated parameters for random sample generation.
///
/// Construct this type with [`SamplesParameters::new`] or [`Default`] so every
/// accepted value can be used to build a sample strategy without violating its
/// partition or sample-ID preconditions.
#[derive(Debug, Clone, Getters)]
pub struct SamplesParameters {
    #[getset(get = "pub")]
    num_different_samples: usize,
    #[getset(get = "pub")]
    num_samples: usize,
    /// The maximum sample ID. This value is inclusive.
    #[getset(get = "pub")]
    max_sample_id: u64,
}

impl SamplesParameters {
    /// Create validated random sample-generation parameters.
    ///
    /// # Errors
    ///
    /// The error chain contains [`SamplesParametersError::TooManyDifferentSamples`]
    /// or [`SamplesParametersError::ZeroDifferentSamples`] if the samples
    /// cannot be partitioned into the requested number of distinct states, or
    /// [`SamplesParametersError::InsufficientSampleIdSpace`] if the inclusive
    /// ID range cannot supply `num_samples` unique IDs.
    pub fn new(
        num_different_samples: usize,
        num_samples: usize,
        max_sample_id: u64,
    ) -> crate::Result<Self> {
        if num_different_samples > num_samples {
            return Err(SamplesParametersError::TooManyDifferentSamples {
                num_different_samples,
                num_samples,
            }
            .into());
        }
        if num_samples > 0 && num_different_samples == 0 {
            return Err(SamplesParametersError::ZeroDifferentSamples { num_samples }.into());
        }

        let sample_id_capacity = u128::from(max_sample_id) + 1;
        if num_samples as u128 > sample_id_capacity {
            return Err(SamplesParametersError::InsufficientSampleIdSpace {
                num_samples,
                max_sample_id,
            }
            .into());
        }
        Ok(Self {
            num_different_samples,
            num_samples,
            max_sample_id,
        })
    }
}

impl Default for SamplesParameters {
    fn default() -> Self {
        Self::new(5, 10, 10).expect("default sample parameters are valid")
    }
}

pub fn arbitrary_samples(
    params: SamplesParameters,
    state_strategy: BoxedStrategy<State>,
) -> BoxedStrategy<Sampled<State>> {
    unique_integers(0, params.max_sample_id, params.num_samples)
        .prop_flat_map(move |sample_ids| {
            let states =
                proptest::collection::vec(state_strategy.clone(), params.num_different_samples);
            let partition =
                arbitrary_integer_partition(sample_ids.len(), params.num_different_samples);
            (states, partition).prop_map(move |(states, partition)| {
                let mut base = 0;
                let mut samples = Sampled::default();
                for (state, size) in states.into_iter().zip(partition) {
                    let ids = sample_ids[base..base + size]
                        .iter()
                        .map(|id| SampleID::from(*id));
                    // Safety: `sample_ids` are unique by construction.
                    samples
                        .append(ids, state)
                        .expect("unique_integers guarantees no duplicate sample IDs");
                    base += size;
                }
                samples
            })
        })
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{random::arbitrary_state, VariableID};

    proptest! {
        #[test]
        fn test_arbitrary_samples(
            samples in arbitrary_samples(
                SamplesParameters::new(10, 100, 1000).unwrap(),
                arbitrary_state((0..=5).map(VariableID::from).collect())
            )
        ) {
            // 100 sample IDs are bucketed into 10 distinct states.
            prop_assert_eq!(samples.num_samples(), 100);
            prop_assert!(samples.ids().iter().all(|id| id.into_inner() <= 1000));
        }
    }

    #[test]
    fn samples_parameters_preserve_too_many_different_samples_signal() {
        let error = SamplesParameters::new(2, 1, 10).unwrap_err();

        assert!(matches!(
            error.downcast_ref::<SamplesParametersError>(),
            Some(SamplesParametersError::TooManyDifferentSamples {
                num_different_samples: 2,
                num_samples: 1,
            })
        ));
    }

    #[test]
    fn samples_parameters_preserve_zero_different_samples_signal() {
        let error = SamplesParameters::new(0, 1, 10).unwrap_err();

        assert!(matches!(
            error.downcast_ref::<SamplesParametersError>(),
            Some(SamplesParametersError::ZeroDifferentSamples { num_samples: 1 })
        ));
    }

    #[test]
    fn samples_parameters_preserve_insufficient_id_space_signal() {
        let error = SamplesParameters::new(1, 2, 0).unwrap_err();

        assert!(matches!(
            error.downcast_ref::<SamplesParametersError>(),
            Some(SamplesParametersError::InsufficientSampleIdSpace {
                num_samples: 2,
                max_sample_id: 0,
            })
        ));
    }

    #[test]
    fn arbitrary_samples_accepts_empty_full_u64_id_space() {
        let params = SamplesParameters::new(0, 0, u64::MAX).unwrap();
        let samples = crate::random::sample_deterministic(arbitrary_samples(
            params,
            arbitrary_state((0..=1).map(VariableID::from).collect()),
        ));

        assert_eq!(samples.num_samples(), 0);
    }

    #[test]
    fn samples_parameters_accept_platform_maximum_in_full_u64_id_space() {
        assert!(SamplesParameters::new(1, usize::MAX, u64::MAX).is_ok());
    }

    #[test]
    fn arbitrary_samples_accepts_nonempty_full_u64_id_space() {
        let params = SamplesParameters::new(1, 1, u64::MAX).unwrap();
        let samples = crate::random::sample_deterministic(arbitrary_samples(
            params,
            arbitrary_state((0..=1).map(VariableID::from).collect()),
        ));

        assert_eq!(samples.num_samples(), 1);
    }

    #[test]
    fn arbitrary_samples_accepts_minimal_nontrivial_partition() {
        let params = SamplesParameters::new(2, 3, 2).unwrap();
        let samples = crate::random::sample_deterministic(arbitrary_samples(
            params,
            arbitrary_state((0..=1).map(VariableID::from).collect()),
        ));

        assert_eq!(samples.num_samples(), 3);
    }
}
