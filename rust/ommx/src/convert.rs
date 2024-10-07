//! Additional trait implementations for generated codes

mod function;
mod linear;
mod polynomial;

use crate::v1::{Quadratic, State};
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
