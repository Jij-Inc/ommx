use crate::{parse::RawParseError, v1};
use derive_more::From;
use std::fmt::Debug;

/// Mathematical function up to polynomial.
///
/// A validated version of [`v1::Function`]. Since the `ommx.v1.Function` is defined by `oneof` in protobuf,
/// it may be `None` if we extend the `Function` enum in the future.
/// Suppose that we add new entry to `ommx.v1.Function`, e.g. `Exponential` or `Logarithm`,
/// and save it as `ommx.v1.Function` in future version of OMMX SDK. This encoded message may be decoded
/// by the current version of OMMX SDK, which does not support the new entry.
/// In this case, the new entry is decoded as `None`.
///
#[derive(Debug, Clone, PartialEq, From)]
pub enum Function {
    Constant(f64),
    Linear(v1::Linear),
    Quadratic(v1::Quadratic),
    Polynomial(v1::Polynomial),
}

impl TryFrom<v1::Function> for Function {
    type Error = RawParseError;

    fn try_from(value: v1::Function) -> Result<Self, Self::Error> {
        match value.function.ok_or(RawParseError::UnsupportedV1Function)? {
            v1::function::Function::Constant(c) => Ok(Function::Constant(c)),
            v1::function::Function::Linear(l) => Ok(Function::Linear(l)),
            v1::function::Function::Quadratic(q) => Ok(Function::Quadratic(q)),
            v1::function::Function::Polynomial(p) => Ok(Function::Polynomial(p)),
        }
    }
}
