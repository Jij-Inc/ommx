//! Additional trait implementations for generated codes

mod function;
mod linear;

use crate::v1::{Polynomial, Quadratic, State};
use std::collections::{BTreeSet, HashMap};

impl From<HashMap<u64, f64>> for State {
    fn from(entries: HashMap<u64, f64>) -> Self {
        Self { entries }
    }
}

impl Quadratic {
    pub fn used_decision_variable_ids(&self) -> BTreeSet<u64> {
        self.columns
            .iter()
            .chain(self.rows.iter())
            .cloned()
            .collect()
    }
}

impl Polynomial {
    pub fn used_decision_variable_ids(&self) -> BTreeSet<u64> {
        self.terms
            .iter()
            .flat_map(|term| term.ids.iter())
            .cloned()
            .collect()
    }
}
