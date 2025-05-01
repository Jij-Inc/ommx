use crate::{
    parse::{Parse, ParseError, RawParseError},
    v1, Coefficient, CoefficientError, Degree, Linear, LinearParameters, MonomialDyn, Polynomial,
    PolynomialParameters, Quadratic, QuadraticParameters,
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

impl Function {
    pub fn num_terms(&self) -> usize {
        match self {
            Function::Zero => 0,
            Function::Constant(_) => 1,
            Function::Linear(l) => l.num_terms(),
            Function::Quadratic(q) => q.num_terms(),
            Function::Polynomial(p) => p.num_terms(),
        }
    }

    pub fn degree(&self) -> Degree {
        match self {
            Function::Zero => 0.into(),
            Function::Constant(_) => 0.into(),
            Function::Linear(l) => l.degree(),
            Function::Quadratic(q) => q.degree(),
            Function::Polynomial(p) => p.degree(),
        }
    }

    pub fn iter(&self) -> Box<dyn Iterator<Item = (MonomialDyn, &Coefficient)> + '_> {
        match self {
            Function::Zero => Box::new(std::iter::empty()),
            Function::Constant(c) => Box::new(std::iter::once((MonomialDyn::default(), c))),
            Function::Linear(l) => Box::new(l.iter().map(|(k, v)| (MonomialDyn::from(*k), v))),
            Function::Quadratic(q) => Box::new(q.iter().map(|(k, v)| (MonomialDyn::from(*k), v))),
            Function::Polynomial(p) => Box::new(p.iter().map(|(k, v)| (k.clone(), v))),
        }
    }

    pub fn iter_mut(&mut self) -> Box<dyn Iterator<Item = (MonomialDyn, &mut Coefficient)> + '_> {
        match self {
            Function::Zero => Box::new(std::iter::empty()),
            Function::Constant(c) => Box::new(std::iter::once((MonomialDyn::default(), c))),
            Function::Linear(l) => Box::new(l.iter_mut().map(|(k, v)| (MonomialDyn::from(*k), v))),
            Function::Quadratic(q) => {
                Box::new(q.iter_mut().map(|(k, v)| (MonomialDyn::from(*k), v)))
            }
            Function::Polynomial(p) => Box::new(p.iter_mut().map(|(k, v)| (k.clone(), v))),
        }
    }

    pub fn values(&self) -> Box<dyn Iterator<Item = &Coefficient> + '_> {
        match self {
            Function::Zero => Box::new(std::iter::empty()),
            Function::Constant(c) => Box::new(std::iter::once(c)),
            Function::Linear(l) => Box::new(l.values()),
            Function::Quadratic(q) => Box::new(q.values()),
            Function::Polynomial(p) => Box::new(p.values()),
        }
    }

    pub fn values_mut(&mut self) -> Box<dyn Iterator<Item = &mut Coefficient> + '_> {
        match self {
            Function::Zero => Box::new(std::iter::empty()),
            Function::Constant(c) => Box::new(std::iter::once(c)),
            Function::Linear(l) => Box::new(l.values_mut()),
            Function::Quadratic(q) => Box::new(q.values_mut()),
            Function::Polynomial(p) => Box::new(p.values_mut()),
        }
    }
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
            .map(Linear::arbitrary_with);
        let quad = if p.max_degree() == 1 {
            linear.clone().map(|l| l.prop_map(Quadratic::from).boxed())
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

#[cfg(test)]
mod tests {
    use super::*;

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
    }
}
