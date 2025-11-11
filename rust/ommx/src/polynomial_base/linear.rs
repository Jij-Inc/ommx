use super::*;
use crate::{random::unique_integers, VariableID, VariableIDPair};
use anyhow::{bail, Result};
use proptest::prelude::*;
use serde::ser::SerializeTuple;
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

impl std::ops::Neg for LinearMonomial {
    type Output = Linear;

    fn neg(self) -> Self::Output {
        Linear::single_term(self, crate::coeff!(-1.0))
    }
}

impl serde::Serialize for LinearMonomial {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            LinearMonomial::Variable(id) => {
                let mut tuple = serializer.serialize_tuple(1)?;
                tuple.serialize_element(&id.into_inner())?;
                tuple.end()
            }
            LinearMonomial::Constant => {
                let tuple = serializer.serialize_tuple(0)?;
                tuple.end()
            }
        }
    }
}

impl<'de> serde::Deserialize<'de> for LinearMonomial {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct LinearMonomialVisitor;

        impl<'de> serde::de::Visitor<'de> for LinearMonomialVisitor {
            type Value = LinearMonomial;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a variable ID (u64) or an array of 0 or 1 variable IDs")
            }

            // When a plain integer is provided, treat it as LinearMonomial::Variable
            fn visit_u64<E>(self, value: u64) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(LinearMonomial::Variable(value.into()))
            }

            // Handle array inputs
            fn visit_seq<A>(self, mut seq: A) -> std::result::Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let first = seq.next_element::<u64>()?;
                let second = seq.next_element::<u64>()?;

                match (first, second) {
                    // Array of length 1 -> LinearMonomial::Variable
                    (Some(id), None) => Ok(LinearMonomial::Variable(id.into())),
                    // Array of length 0 -> LinearMonomial::Constant
                    (None, None) => Ok(LinearMonomial::Constant),
                    // Any other length is an error
                    _ => Err(serde::de::Error::custom("expected array of length 0 or 1")),
                }
            }
        }

        deserializer.deserialize_any(LinearMonomialVisitor)
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

    fn reduce_binary_power(&mut self, _: &VariableIDSet) -> bool {
        // Linear monomials are already linear, so no reduction is needed.
        false
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

    #[test]
    fn test_linear_monomial_serde() {
        // Test Variable serialization/deserialization with u64
        let var = LinearMonomial::Variable(42.into());
        let json = serde_json::to_string(&var).unwrap();
        assert_eq!(json, "[42]");
        let deserialized: LinearMonomial = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, var);

        // Test deserializing from plain u64
        let deserialized: LinearMonomial = serde_json::from_str("42").unwrap();
        assert_eq!(deserialized, LinearMonomial::Variable(42.into()));

        // Test Constant serialization/deserialization
        let constant = LinearMonomial::Constant;
        let json = serde_json::to_string(&constant).unwrap();
        assert_eq!(json, "[]");
        let deserialized: LinearMonomial = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, constant);

        // Test round-trip for Variable
        let original = LinearMonomial::Variable(123.into());
        let serialized = serde_json::to_string(&original).unwrap();
        let deserialized: LinearMonomial = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, original);

        // Test round-trip for Constant
        let original = LinearMonomial::Constant;
        let serialized = serde_json::to_string(&original).unwrap();
        let deserialized: LinearMonomial = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, original);
    }

    #[test]
    fn test_linear_monomial_deserialize_invalid() {
        // Array with more than 1 element should fail
        let result: Result<LinearMonomial, _> = serde_json::from_str("[1, 2]");
        assert!(result.is_err());

        // Array with more than 2 elements should fail
        let result: Result<LinearMonomial, _> = serde_json::from_str("[1, 2, 3]");
        assert!(result.is_err());
    }
}
