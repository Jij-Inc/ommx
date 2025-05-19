mod arbitrary;
mod parse;

pub use arbitrary::*;
use getset::CopyGetters;

use crate::Bound;
use derive_more::{Deref, From};
use fnv::FnvHashMap;
use std::collections::BTreeSet;

/// ID for decision variable and parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, From, Deref)]
pub struct VariableID(u64);
pub type VariableIDSet = BTreeSet<VariableID>;

impl VariableID {
    pub fn into_inner(&self) -> u64 {
        self.0
    }
}

impl From<VariableID> for u64 {
    fn from(id: VariableID) -> Self {
        id.0
    }
}

impl std::fmt::Display for VariableID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    Continuous,
    Integer,
    Binary,
    SemiContinuous,
    SemiInteger,
}

/// The decision variable with metadata.
///
/// Invariants
/// ----------
/// - At least one possible value exists for the pair of `kind` and `bound`.
///   - If `kind` is `Kind::Integer`, then `bound` must contains at least one integer.
///     e.g. `kind = Kind::Integer` and `bound = [1.1, 1.9]` is invalid state.
///   - If `kind` is `Kind::Binary`, then `bound` must contains one of `0.0` or `1.0`.
///
#[derive(Debug, Clone, PartialEq, CopyGetters)]
pub struct DecisionVariable {
    #[getset(get_copy = "pub")]
    id: VariableID,
    #[getset(get_copy = "pub")]
    kind: Kind,
    #[getset(get_copy = "pub")]
    bound: Bound,
    #[getset(get_copy = "pub")]
    substituted_value: Option<f64>,

    pub name: Option<String>,
    pub subscripts: Vec<i64>,
    pub parameters: FnvHashMap<String, String>,
    pub description: Option<String>,
}

impl DecisionVariable {
    /// Create a new decision variable.
    pub fn new(
        id: VariableID,
        kind: Kind,
        bound: Bound,
        substituted_value: Option<f64>,
    ) -> Result<Self, DecisionVariableError> {
        Ok(Self {
            id,
            kind,
            bound,
            substituted_value,
            name: None,
            subscripts: Vec::new(),
            parameters: FnvHashMap::default(),
            description: None,
        })
    }

    pub fn set_bound(&mut self, bound: Bound) -> Result<(), DecisionVariableError> {
        todo!()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DecisionVariableError {
    #[error("Bound for ID={id} is inconsistent to kind: kind={kind:?}, bound={bound}")]
    BoundInconsistent {
        id: VariableID,
        kind: Kind,
        bound: Bound,
    },

    #[error("Substituted variable for ID={id} is inconsistent: previous={previous_value}, new={new_value}")]
    SubstitutedVariableInconsistent {
        id: VariableID,
        previous_value: f64,
        new_value: f64,
    },
}
