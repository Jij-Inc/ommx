use crate::{
    parse::{Parse, ParseError, RawParseError},
    v1, Coefficient, CoefficientError, Linear, LinearParameters, Polynomial, PolynomialParameters,
    Quadratic, QuadraticParameters,
};
use derive_more::From;
use num::Zero;
use proptest::{prelude::*, strategy::Union};
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

impl TryFrom<v1::Function> for Function {
    type Error = ParseError;
    fn try_from(value: v1::Function) -> Result<Self, Self::Error> {
        value.parse(&())
    }
}

impl From<Function> for v1::Function {
    fn from(value: Function) -> Self {
        use v1::function::Function::*;
        let function = match value {
            Function::Zero => Constant(0.0),
            Function::Constant(c) => Constant(c.into()),
            Function::Linear(l) => Linear(l.into()),
            Function::Quadratic(q) => Quadratic(q.into()),
            Function::Polynomial(p) => Polynomial(p.into()),
        };
        Self {
            function: Some(function),
        }
    }
}

impl Arbitrary for Function {
    type Parameters = PolynomialParameters;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(p: Self::Parameters) -> Self::Strategy {
        if p.num_terms() == 0 {
            return prop_oneof![
                Just(Function::Zero),
                Just(Function::Linear(Linear::zero())),
                Just(Function::Quadratic(Quadratic::zero())),
                Just(Function::Polynomial(Polynomial::zero())),
            ]
            .boxed();
        }
        if p.max_degree() == 0 {
            debug_assert_eq!(p.num_terms(), 1);
            return Coefficient::arbitrary()
                .prop_map(Function::Constant)
                .boxed();
        }
        let polynomial = Polynomial::arbitrary_with(p);
        let linear = LinearParameters::new(p.num_terms(), p.max_id())
            .ok()
            .map(|p| Linear::arbitrary_with(p));
        let quad = if p.max_degree() == 1 {
            linear
                .clone()
                .map(|l| l.prop_map(|l| Quadratic::from(l)).boxed())
        } else {
            QuadraticParameters::new(p.num_terms(), p.max_id())
                .ok()
                .map(|p| Quadratic::arbitrary_with(p).boxed())
        };
        let mut candidates = vec![polynomial.prop_map(Function::Polynomial).boxed()];
        if let Some(l) = linear {
            candidates.push(l.prop_map(Function::Linear).boxed());
        }
        if let Some(q) = quad {
            candidates.push(q.prop_map(Function::Quadratic).boxed());
        }
        Union::new(candidates).boxed()
    }
}
