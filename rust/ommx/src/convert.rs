//! Additional trait implementations for generated codes

use crate::v1::{
    function::{self, Function as FunctionEnum},
    linear::Term,
    Function, Linear, Polynomial, Quadratic, State,
};
use std::collections::{BTreeSet, HashMap};

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

impl Function {
    pub fn used_decision_variable_ids(&self) -> BTreeSet<u64> {
        match &self.function {
            Some(FunctionEnum::Linear(linear)) => linear.used_decision_variable_ids(),
            Some(FunctionEnum::Quadratic(quadratic)) => quadratic.used_decision_variable_ids(),
            Some(FunctionEnum::Polynomial(poly)) => poly.used_decision_variable_ids(),
            _ => BTreeSet::new(),
        }
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

    pub fn used_decision_variable_ids(&self) -> BTreeSet<u64> {
        self.terms.iter().map(|term| term.id).collect()
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
