use super::*;
use crate::{random::unique_integers, VariableID, VariableIDPair};
use anyhow::{bail, Result};
use proptest::prelude::*;
use std::{collections::HashSet, fmt::Debug, hash::Hash};

pub type Linear = PolynomialBase<LinearMonomial>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, getset::CopyGetters)]
pub struct LinearParameters {
    #[getset(get_copy = "pub")]
    num_terms: usize,
    #[getset(get_copy = "pub")]
    /// This ID is allowed. So when the `max_id=2`, `[0, 1, 2]` are allowed.
    max_id: VariableID,
}

impl LinearParameters {
    pub fn new(num_terms: usize, max_id: VariableID) -> Result<Self> {
        if num_terms > Into::<u64>::into(max_id) as usize + 2 {
            bail!("num_terms{num_terms} cannot be greater than max_id({max_id}) + 2");
        }
        Ok(Self { num_terms, max_id })
    }

    pub fn full(max_id: VariableID) -> Self {
        Self {
            num_terms: Into::<u64>::into(max_id) as usize + 2,
            max_id,
        }
    }

    /// There is one possible output.
    pub fn is_full(&self) -> bool {
        Into::<u64>::into(self.max_id) as usize + 2 == self.num_terms
    }

    pub fn is_empty(&self) -> bool {
        self.num_terms == 0
    }
}

impl Arbitrary for LinearParameters {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        (0..10_usize)
            .prop_flat_map(|num_terms| {
                let minimal_possible_max_id = if num_terms < 2 {
                    0
                } else {
                    num_terms as u64 - 2
                };
                (minimal_possible_max_id..=10).prop_map(move |max_id| {
                    LinearParameters::new(num_terms, max_id.into()).unwrap()
                })
            })
            .boxed()
    }
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

impl LinearMonomial {
    pub fn iter(&self) -> Box<dyn Iterator<Item = VariableID>> {
        match self {
            LinearMonomial::Variable(id) => Box::new(std::iter::once(*id)),
            LinearMonomial::Constant => Box::new(std::iter::empty()),
        }
    }
}

impl From<VariableID> for LinearMonomial {
    fn from(value: VariableID) -> Self {
        LinearMonomial::Variable(value)
    }
}

impl From<u64> for LinearMonomial {
    fn from(value: u64) -> Self {
        LinearMonomial::Variable(VariableID::from(value))
    }
}

impl Monomial for LinearMonomial {
    type Parameters = LinearParameters;

    fn degree(&self) -> Degree {
        match self {
            LinearMonomial::Variable(_) => 1.into(),
            LinearMonomial::Constant => 0.into(),
        }
    }

    fn max_degree() -> Degree {
        1.into()
    }

    fn as_linear(&self) -> Option<VariableID> {
        match self {
            LinearMonomial::Variable(id) => Some(*id),
            LinearMonomial::Constant => None,
        }
    }

    fn as_quadratic(&self) -> Option<VariableIDPair> {
        None
    }

    fn ids(&self) -> Box<dyn Iterator<Item = VariableID>> {
        match self {
            LinearMonomial::Variable(id) => Box::new(std::iter::once(*id)),
            LinearMonomial::Constant => Box::new(std::iter::empty()),
        }
    }

    fn from_ids(mut ids: impl Iterator<Item = VariableID>) -> Option<Self> {
        match (ids.next(), ids.next()) {
            (Some(id), None) => Some(LinearMonomial::Variable(id)),
            (None, None) => Some(LinearMonomial::Constant),
            _ => None,
        }
    }

    fn partial_evaluate(self, state: &State) -> (Self, f64) {
        if let LinearMonomial::Variable(id) = self {
            if let Some(value) = state.entries.get(&id.into_inner()) {
                return (Self::default(), *value);
            }
        }
        (self, 1.0)
    }

    fn arbitrary_uniques(p: LinearParameters) -> BoxedStrategy<FnvHashSet<Self>> {
        if p.is_empty() {
            return Just(HashSet::default()).boxed();
        }
        let max_id = p.max_id.into();
        if p.is_full() {
            return Just(
                (0..=max_id)
                    .map(|id| LinearMonomial::Variable(id.into()))
                    .chain(std::iter::once(LinearMonomial::Constant))
                    .collect(),
            )
            .boxed();
        }
        // Since the parameter is not full, we can randomly select the constant is zero or finite
        bool::arbitrary()
            .prop_flat_map(move |use_constant| {
                if use_constant {
                    unique_integers(0, max_id, p.num_terms - 1)
                        .prop_map(|ids| {
                            ids.into_iter()
                                .map(|id| LinearMonomial::Variable(id.into()))
                                .chain(std::iter::once(LinearMonomial::Constant))
                                .collect()
                        })
                        .boxed()
                } else {
                    unique_integers(0, max_id, p.num_terms)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unacceptable_parameter() {
        // Possible term is [constant, 0, 1, 2], cannot be 5
        assert!(LinearParameters::new(5, 2.into()).is_err());
        assert!(LinearParameters::new(5, 3.into()).is_ok());
    }

    proptest! {
        #[test]
        fn test_linear(
            (p, monomials) in LinearParameters::arbitrary()
                .prop_flat_map(|p| {
                    LinearMonomial::arbitrary_uniques(p)
                        .prop_map(move |monomials| (p, monomials))
                }),
        ) {
            prop_assert_eq!(monomials.len(), p.num_terms);
            for monomial in monomials {
                match monomial {
                    LinearMonomial::Variable(id) => {
                        prop_assert!(id <= p.max_id);
                    }
                    LinearMonomial::Constant => {}
                }
            }
        }
    }
}
