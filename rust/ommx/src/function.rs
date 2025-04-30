use crate::{
    parse::{Parse, ParseError, RawParseError},
    v1, Coefficient, CoefficientError, Linear, Polynomial, Quadratic,
};
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
    Zero,
    /// Non-zero constant
    Constant(Coefficient),
    Linear(Linear),
    Quadratic(Quadratic),
    Polynomial(Polynomial),
}

impl Parse for v1::Function {
    type Output = Function;
    type Context = ();
    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.Function";
        use v1::function::Function::*;
        match self.function.ok_or(RawParseError::UnsupportedV1Function)? {
            Constant(c) => match c.try_into() {
                Ok(c) => Ok(Function::Constant(c)),
                Err(CoefficientError::Zero) => Ok(Function::Zero),
                Err(c) => Err(RawParseError::from(c).context(message, "constant")),
            },
            Linear(l) => Ok(Function::Linear(l.parse_as(&(), message, "linear")?)),
            Quadratic(q) => Ok(Function::Quadratic(q.parse_as(
                &(),
                message,
                "quadratic",
            )?)),
            Polynomial(p) => Ok(Function::Polynomial(p.parse_as(
                &(),
                message,
                "polynomial",
            )?)),
        }
    }
}
