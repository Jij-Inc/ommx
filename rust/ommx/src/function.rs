use crate::{error::Error, v1};
use derive_more::From;

/// A mathematical function up to polynomial.
#[derive(Debug, Clone, PartialEq, From)]
pub enum Function {
    Constant(f64),
    Linear(v1::Linear),
    Quadratic(v1::Quadratic),
    Polynomial(v1::Polynomial),
}

impl TryFrom<v1::Function> for Function {
    type Error = Error;

    fn try_from(value: v1::Function) -> Result<Self, Self::Error> {
        match value.function.ok_or(Error::UnsupportedV1Function)? {
            v1::function::Function::Constant(c) => Ok(Self::Constant(c)),
            v1::function::Function::Linear(l) => Ok(Self::Linear(l)),
            v1::function::Function::Quadratic(q) => Ok(Self::Quadratic(q)),
            v1::function::Function::Polynomial(p) => Ok(Self::Polynomial(p)),
        }
    }
}

impl From<Function> for v1::Function {
    fn from(value: Function) -> Self {
        let function = match value {
            Function::Constant(c) => v1::function::Function::Constant(c),
            Function::Linear(l) => v1::function::Function::Linear(l),
            Function::Quadratic(q) => v1::function::Function::Quadratic(q),
            Function::Polynomial(p) => v1::function::Function::Polynomial(p),
        };

        Self {
            function: Some(function),
        }
    }
}
