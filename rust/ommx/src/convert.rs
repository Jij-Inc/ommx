//! Additional trait implementations for generated codes

mod function;
mod linear;
mod polynomial;
mod quadratic;

use crate::v1::State;
use std::collections::HashMap;

impl From<HashMap<u64, f64>> for State {
    fn from(entries: HashMap<u64, f64>) -> Self {
        Self { entries }
    }
}
