mod arbitrary;
mod parse;

pub use arbitrary::*;

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

#[derive(Debug, Clone, PartialEq)]
pub struct DecisionVariable {
    pub id: VariableID,
    pub kind: Kind,
    pub bound: Bound,

    pub substituted_value: Option<f64>,

    pub name: Option<String>,
    pub subscripts: Vec<i64>,
    pub parameters: FnvHashMap<String, String>,
    pub description: Option<String>,
}
