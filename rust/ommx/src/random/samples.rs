use crate::{
    random::{arbitrary_integer_partition, unique_integers},
    v1::State,
    SampleID, Sampled,
};
use anyhow::{bail, Result};
use getset::Getters;
use proptest::prelude::*;

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
    pub fn new(
        num_different_samples: usize,
        num_samples: usize,
        max_sample_id: u64,
    ) -> Result<Self> {
        if num_different_samples > num_samples {
            bail!(
                "num_different_samples({num_different_samples}) must be less than or equal to num_samples({num_samples})."
            );
        }
        if num_samples > max_sample_id as usize + 1 {
            bail!(
                "num_samples({num_samples}) must be less than max_sample_id({max_sample_id}) + 1."
            );
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
        Self {
            num_different_samples: 5,
            num_samples: 10,
            max_sample_id: 10,
        }
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
}
