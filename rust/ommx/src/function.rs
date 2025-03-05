use crate::{error::ParseError, v1};
use derive_more::From;
use std::fmt::Debug;

#[derive(Debug, Clone, PartialEq, From)]
pub enum Function {
    Constant(f64),
    Linear(v1::Linear),
    Quadratic(v1::Quadratic),
    Polynomial(v1::Polynomial),
}

impl TryFrom<v1::Function> for Function {
    type Error = ParseError;

    fn try_from(value: v1::Function) -> Result<Self, Self::Error> {
        match value.function.ok_or(ParseError::UnsupportedV1Function)? {
            v1::function::Function::Constant(c) => Ok(Function::Constant(c)),
            v1::function::Function::Linear(l) => Ok(Function::Linear(l)),
            v1::function::Function::Quadratic(q) => Ok(Function::Quadratic(q)),
            v1::function::Function::Polynomial(p) => Ok(Function::Polynomial(p)),
        }
    }
}
