use super::*;
use crate::{
    parse::{Parse, ParseError, RawParseError},
    v1, CoefficientError,
};

/// Note for [`Parse`] implementation
/// ----------------------------------
/// Since the `ommx.v1.Function` is defined by `oneof` in protobuf,
/// it may be `None` if we extend the `Function` enum in the future.
/// Suppose that we add new entry to `ommx.v1.Function`, e.g. `Exponential` or `Logarithm`,
/// and save it as `ommx.v1.Function` in future version of OMMX SDK. This encoded message may be decoded
/// by the current version of OMMX SDK, which does not support the new entry.
/// In this case, the new entry is decoded as `None`.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PolynomialParameters;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_function(
            (p, function) in PolynomialParameters::arbitrary()
                .prop_flat_map(|p| {
                    Function::arbitrary_with(p)
                        .prop_map(move |function| (p, function))
                }),
        ) {
            prop_assert_eq!(function.num_terms(), p.num_terms());
            prop_assert!(function.degree() <= p.max_degree());
            for (monomial, _) in function.iter() {
                for id in monomial.iter() {
                    prop_assert!(*id <= p.max_id().into_inner());
                }
            }
        }

        /// Function -> v1::Function -> Function
        #[test]
        fn test_function_roundtrip(original in Function::arbitrary()) {
            let v1_function = v1::Function::try_from(original.clone()).unwrap();
            let parsed = Function::try_from(v1_function).unwrap();
            prop_assert_eq!(original, parsed);
        }
    }
}
