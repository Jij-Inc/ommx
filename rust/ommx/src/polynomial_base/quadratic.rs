use super::*;
use crate::{
    random::{multi_choose, unique_integer_pairs},
    Monomial, VariableID,
};
use anyhow::{bail, Result};
use derive_more::From;
use proptest::prelude::*;

pub type Quadratic = PolynomialBase<QuadraticMonomial>;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, From)]
pub enum QuadraticMonomial {
    Pair(VariableIDPair),
    Linear(VariableID),
    #[default]
    Constant,
}

impl From<(VariableID, VariableID)> for QuadraticMonomial {
    fn from(pair: (VariableID, VariableID)) -> Self {
        Self::new_pair(pair.0, pair.1)
    }
}

impl QuadraticMonomial {
    pub fn new_pair(a: VariableID, b: VariableID) -> Self {
        Self::Pair(VariableIDPair::new(a, b))
    }

    pub fn iter(&self) -> Box<dyn Iterator<Item = VariableID>> {
        match self {
            Self::Pair(pair) => Box::new(pair.iter()),
            Self::Linear(id) => Box::new(std::iter::once(*id)),
            Self::Constant => Box::new(std::iter::empty()),
        }
    }
}

impl From<LinearMonomial> for QuadraticMonomial {
    fn from(m: LinearMonomial) -> Self {
        match m {
            LinearMonomial::Variable(id) => Self::Linear(id),
            LinearMonomial::Constant => Self::Constant,
        }
    }
}

impl TryFrom<&QuadraticMonomial> for LinearMonomial {
    type Error = InvalidDegreeError;
    fn try_from(m: &QuadraticMonomial) -> std::result::Result<Self, InvalidDegreeError> {
        match m {
            QuadraticMonomial::Pair(_) => Err(InvalidDegreeError {
                degree: 2.into(),
                max_degree: 1.into(),
            }),
            QuadraticMonomial::Linear(id) => Ok(LinearMonomial::from(*id)),
            QuadraticMonomial::Constant => Ok(LinearMonomial::Constant),
        }
    }
}

impl From<Linear> for Quadratic {
    fn from(l: Linear) -> Self {
        Self {
            terms: l.terms.into_iter().map(|(k, v)| (k.into(), v)).collect(),
        }
    }
}

impl Quadratic {
    /// Create a new quadratic from lists of columns, rows, and values
    pub fn from_coo(
        columns: impl IntoIterator<Item = VariableID>,
        rows: impl IntoIterator<Item = VariableID>,
        values: impl IntoIterator<Item = Coefficient>,
    ) -> Result<Self> {
        let mut result = Self::default();
        let mut columns = columns.into_iter();
        let mut rows = rows.into_iter();
        let mut values = values.into_iter();
        loop {
            match (columns.next(), rows.next(), values.next()) {
                (Some(col), Some(row), Some(val)) => {
                    let pair = VariableIDPair::new(col, row);
                    result.add_term(QuadraticMonomial::Pair(pair), val);
                }
                (None, None, None) => break,
                _ => bail!("Mismatched lengths of columns, rows, and values"),
            }
        }
        Ok(result)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VariableIDPair {
    lower: VariableID,
    upper: VariableID,
}

impl VariableIDPair {
    pub fn new(a: VariableID, b: VariableID) -> Self {
        if a <= b {
            Self { lower: a, upper: b }
        } else {
            Self { lower: b, upper: a }
        }
    }

    pub fn lower(&self) -> VariableID {
        self.lower
    }

    pub fn upper(&self) -> VariableID {
        self.upper
    }

    pub fn iter(&self) -> impl Iterator<Item = VariableID> {
        std::iter::once(self.lower).chain(std::iter::once(self.upper))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, getset::CopyGetters)]
pub struct QuadraticParameters {
    #[getset(get_copy = "pub")]
    num_terms: usize,
    /// This ID is allowed. So when the `max_id=2`, `[0, 1, 2]` are allowed.
    #[getset(get_copy = "pub")]
    max_id: VariableID,
}

impl QuadraticParameters {
    pub fn new(num_terms: usize, max_id: VariableID) -> Result<Self> {
        let test = Self { num_terms, max_id };
        if num_terms > test.largest_max_degree_terms() + test.largest_sub_degree_terms() {
            bail!("Cannot create {num_terms} terms in quadratic polynomial with `max_id={max_id}`");
        }
        Ok(test)
    }

    fn largest_max_degree_terms(&self) -> usize {
        multi_choose(self.max_id.into_inner() + 1, 2) as usize
    }

    fn largest_sub_degree_terms(&self) -> usize {
        let linear = self.max_id.into_inner() as usize + 1;
        linear + 1 /* constant */
    }
}

impl Arbitrary for QuadraticParameters {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;
    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        (0..=10_u64)
            .prop_flat_map(move |max_id| {
                let p = Self {
                    num_terms: 0,
                    max_id: max_id.into(),
                };
                let max_num_terms = p.largest_max_degree_terms() + p.largest_sub_degree_terms();
                (0..=max_num_terms)
                    .prop_map(move |num_terms| Self::new(num_terms, max_id.into()).unwrap())
            })
            .boxed()
    }
}

impl Default for QuadraticParameters {
    fn default() -> Self {
        Self {
            num_terms: 5,
            max_id: 10.into(),
        }
    }
}

impl Monomial for QuadraticMonomial {
    type Parameters = QuadraticParameters;

    fn degree(&self) -> Degree {
        match self {
            Self::Pair(_) => 2.into(),
            Self::Linear(_) => 1.into(),
            Self::Constant => 0.into(),
        }
    }

    fn max_degree() -> Degree {
        2.into()
    }

    fn as_linear(&self) -> Option<VariableID> {
        match self {
            Self::Linear(id) => Some(*id),
            _ => None,
        }
    }

    fn as_quadratic(&self) -> Option<VariableIDPair> {
        match self {
            Self::Pair(pair) => Some(*pair),
            _ => None,
        }
    }

    fn reduce_binary_power(&mut self, binary_ids: &VariableIDSet) -> bool {
        if let Self::Pair(VariableIDPair { lower, upper }) = self {
            if lower != upper {
                // If the pair is not the same, we cannot reduce it.
                return false;
            }
            if binary_ids.contains(lower) {
                // If both IDs are binary, we can reduce it to linear.
                *self = Self::Linear(*lower);
                return true;
            }
        }
        false
    }

    fn ids(&self) -> Box<dyn Iterator<Item = VariableID> + '_> {
        match self {
            Self::Pair(pair) => Box::new(pair.iter()),
            Self::Linear(id) => Box::new(std::iter::once(*id)),
            Self::Constant => Box::new(std::iter::empty()),
        }
    }

    fn from_ids(mut ids: impl Iterator<Item = VariableID>) -> Option<Self> {
        match (ids.next(), ids.next(), ids.next()) {
            (Some(a), Some(b), None) => Some(Self::new_pair(a, b)),
            (Some(a), None, None) => Some(Self::Linear(a)),
            (None, None, None) => Some(Self::Constant),
            _ => None,
        }
    }

    fn partial_evaluate(self, state: &State) -> (Self, f64) {
        match self {
            Self::Pair(VariableIDPair { lower, upper }) => {
                let lower = lower.into_inner();
                let upper = upper.into_inner();
                match (state.entries.get(&lower), state.entries.get(&upper)) {
                    (Some(l), Some(u)) => {
                        return (Self::default(), (*l) * (*u));
                    }
                    (Some(l), None) => {
                        return (Self::Linear(upper.into()), *l);
                    }
                    (None, Some(u)) => {
                        return (Self::Linear(lower.into()), *u);
                    }
                    _ => {}
                }
            }
            Self::Linear(id) => {
                if let Some(value) = state.entries.get(&id.into_inner()) {
                    return (Self::default(), *value);
                }
            }
            _ => {}
        }
        (self, 1.0)
    }

    fn arbitrary_uniques(p: Self::Parameters) -> BoxedStrategy<FnvHashSet<Self>> {
        let min = if p.num_terms >= p.largest_sub_degree_terms() {
            p.num_terms - p.largest_sub_degree_terms()
        } else {
            0
        };
        let max = p.largest_max_degree_terms().min(p.num_terms);
        (min..=max)
            .prop_flat_map(move |num_quad| {
                let ids = unique_integer_pairs(p.max_id.into_inner(), num_quad);
                let linear = LinearMonomial::arbitrary_uniques(
                    LinearParameters::new(p.num_terms - num_quad, p.max_id).unwrap(),
                );
                (ids, linear).prop_map(|(ids, sub)| {
                    sub.into_iter()
                        .map(|id| id.into())
                        .chain(
                            ids.into_iter()
                                .map(|(a, b)| QuadraticMonomial::new_pair(a.into(), b.into())),
                        )
                        .collect()
                })
            })
            .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    proptest! {
        #[test]
        fn test_quadratic(
            (p, monomials) in QuadraticParameters::arbitrary()
                .prop_flat_map(|p| {
                    QuadraticMonomial::arbitrary_uniques(p)
                        .prop_map(move |monomials| (p, monomials))
                }),
        ) {
            prop_assert_eq!(monomials.len(), p.num_terms);
            for monomial in monomials {
                for id in monomial.iter() {
                    prop_assert!(*id <= p.max_id.into_inner());
                }
            }
        }
    }
}
