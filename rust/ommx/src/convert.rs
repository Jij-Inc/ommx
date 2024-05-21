//! Additional trait implementations for generated codes

use crate::v1::{function, linear::Term, Function, Linear, Quadratic, State};
use std::collections::HashMap;

impl From<function::Function> for Function {
    fn from(f: function::Function) -> Self {
        Self { function: Some(f) }
    }
}

impl From<Linear> for Function {
    fn from(linear: Linear) -> Self {
        Self {
            function: Some(function::Function::Linear(linear)),
        }
    }
}

impl From<Quadratic> for Function {
    fn from(q: Quadratic) -> Self {
        Self {
            function: Some(function::Function::Quadratic(q)),
        }
    }
}

impl From<HashMap<u64, f64>> for State {
    fn from(entries: HashMap<u64, f64>) -> Self {
        Self { entries }
    }
}

impl Linear {
    pub fn new(terms: impl Iterator<Item = (u64, f64)>, constant: f64) -> Self {
        Self {
            terms: terms
                .map(|(id, coefficient)| Term { id, coefficient })
                .collect(),
            constant,
        }
    }
}
