//! Additional trait implementations for generated codes

use crate::v1::{function, Function, Linear, Quadratic};

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
