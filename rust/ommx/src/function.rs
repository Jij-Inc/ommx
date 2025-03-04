use crate::{error::Error, v1};
use std::fmt::Debug;

/// Trait for a mathematical function up to polynomial
///
/// Note that this trait does not inherits `Add` or `Zero` to keep object-safety.
///
pub trait Function: Debug {
    /// Degree of the polynomial for non-zero polynomials, and 0 for zero polynomials.
    fn degree(&self) -> u32;
}

impl TryFrom<v1::Function> for Box<dyn Function> {
    type Error = Error;
    fn try_from(value: v1::Function) -> Result<Self, Self::Error> {
        match value.function.ok_or(Error::UnsupportedV1Function)? {
            v1::function::Function::Constant(c) => Ok(Box::new(c)),
            v1::function::Function::Linear(l) => Ok(Box::new(l)),
            v1::function::Function::Quadratic(q) => Ok(Box::new(q)),
            v1::function::Function::Polynomial(p) => Ok(Box::new(p)),
        }
    }
}

impl Function for f64 {
    fn degree(&self) -> u32 {
        0
    }
}

impl Function for v1::Linear {
    fn degree(&self) -> u32 {
        self.degree()
    }
}

impl Function for v1::Quadratic {
    fn degree(&self) -> u32 {
        self.degree()
    }
}

impl Function for v1::Polynomial {
    fn degree(&self) -> u32 {
        self.degree()
    }
}
