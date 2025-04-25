use super::*;
use crate::{random::unique_integers, VariableID};
use proptest::prelude::*;
use std::{fmt::Debug, hash::Hash};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinearParameters {
    num_terms: usize,
    max_id: VariableID,
}

impl Default for LinearParameters {
    fn default() -> Self {
        Self {
            num_terms: 3,
            max_id: 10.into(),
        }
    }
}

/// Linear function only contains monomial of degree 1 or constant
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LinearMonomial {
    Variable(VariableID),
    #[default]
    Constant,
}

impl Monomial for LinearMonomial {
    type Parameters = LinearParameters;
    fn arbitrary_distinct(
        LinearParameters { num_terms, max_id }: LinearParameters,
    ) -> BoxedStrategy<Vec<Self>> {
        if num_terms == 0 {
            return Just(Vec::new()).boxed();
        }
        let max_id = max_id.into();
        if num_terms as u64 == max_id + 1 {
            return Just(
                (0..max_id)
                    .map(|id| LinearMonomial::Variable(id.into()))
                    .chain(std::iter::once(LinearMonomial::Constant))
                    .collect(),
            )
            .boxed();
        }
        bool::arbitrary()
            .prop_flat_map(move |use_constant| {
                if use_constant {
                    unique_integers(0, max_id, num_terms - 1)
                        .prop_map(|ids| {
                            ids.into_iter()
                                .map(|id| LinearMonomial::Variable(id.into()))
                                .chain(std::iter::once(LinearMonomial::Constant))
                                .collect()
                        })
                        .boxed()
                } else {
                    unique_integers(0, max_id, num_terms)
                        .prop_map(|ids| {
                            ids.into_iter()
                                .map(|id| LinearMonomial::Variable(id.into()))
                                .collect()
                        })
                        .boxed()
                }
            })
            .boxed()
    }
}
