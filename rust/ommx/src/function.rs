use crate::{
    Coefficient, Degree, Linear, MonomialDyn, Polynomial, Quadratic, VariableID, VariableIDPair,
};
use derive_more::From;
use num::{traits::Inv, One, Zero};
use std::{borrow::Cow, fmt::Debug};

mod add;
mod approx;
mod arbitrary;
mod evaluate;
mod mul;
mod parse;
mod reduce_binary_power;
mod sub;
mod substitute;

/// A real-valued function of decision variables used for objective and constraint functions.
///
/// This can be up to polynomial currently, but it will be extended to exponential and logarithm in the future.
#[derive(Debug, Clone, PartialEq, From, Default)]
pub enum Function {
    #[default]
    Zero,
    /// Non-zero constant
    Constant(Coefficient),
    Linear(Linear),
    Quadratic(Quadratic),
    Polynomial(Polynomial),
}

impl Function {
    pub fn constant_term(&self) -> f64 {
        match self {
            Function::Zero => 0.0,
            Function::Constant(c) => c.into_inner(),
            Function::Linear(l) => l.constant_term(),
            Function::Quadratic(q) => q.constant_term(),
            Function::Polynomial(p) => p.constant_term(),
        }
    }

    pub fn linear_terms(&self) -> Box<dyn Iterator<Item = (VariableID, Coefficient)> + '_> {
        match self {
            Function::Zero => Box::new(std::iter::empty()),
            Function::Constant(_) => Box::new(std::iter::empty()),
            Function::Linear(l) => Box::new(l.linear_terms()),
            Function::Quadratic(q) => Box::new(q.linear_terms()),
            Function::Polynomial(p) => Box::new(p.linear_terms()),
        }
    }

    pub fn quadratic_terms(&self) -> Box<dyn Iterator<Item = (VariableIDPair, Coefficient)> + '_> {
        match self {
            Function::Zero => Box::new(std::iter::empty()),
            Function::Constant(_) => Box::new(std::iter::empty()),
            Function::Linear(l) => Box::new(l.quadratic_terms()),
            Function::Quadratic(q) => Box::new(q.quadratic_terms()),
            Function::Polynomial(p) => Box::new(p.quadratic_terms()),
        }
    }

    pub fn as_linear(&self) -> Option<Cow<Linear>> {
        match self {
            Function::Zero => Some(Cow::Owned(Linear::zero())),
            Function::Constant(c) => Some(Cow::Owned((*c).into())),
            Function::Linear(l) => Some(Cow::Borrowed(l)),
            Function::Quadratic(q) => q.try_into().map(Cow::Owned).ok(),
            Function::Polynomial(p) => p.try_into().map(Cow::Owned).ok(),
        }
    }

    pub fn as_quadratic(&self) -> Option<Cow<Quadratic>> {
        match self {
            Function::Zero => Some(Cow::Owned(Quadratic::zero())),
            Function::Constant(c) => Some(Cow::Owned((*c).into())),
            Function::Linear(l) => Some(Cow::Owned(l.clone().into())),
            Function::Quadratic(q) => Some(Cow::Borrowed(q)),
            Function::Polynomial(p) => p.try_into().map(Cow::Owned).ok(),
        }
    }

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

    pub fn keys(&self) -> Box<dyn Iterator<Item = MonomialDyn> + '_> {
        match self {
            Function::Zero => Box::new(std::iter::empty()),
            Function::Constant(_) => Box::new(std::iter::once(MonomialDyn::default())),
            Function::Linear(l) => Box::new(l.keys().map(|k| MonomialDyn::from(*k))),
            Function::Quadratic(q) => Box::new(q.keys().map(|k| MonomialDyn::from(*k))),
            Function::Polynomial(p) => Box::new(p.keys().cloned()),
        }
    }

    /// Get a minimal positive factor `a` which make all coefficients of `a * self` integer.
    ///
    /// This returns `1` for zero function. See also <https://en.wikipedia.org/wiki/Primitive_part_and_content>.
    pub fn content_factor(&self) -> anyhow::Result<Coefficient> {
        match self {
            Function::Zero => Ok(Coefficient::one()),
            Function::Constant(c) => Ok(c.inv()),
            Function::Linear(l) => l.content_factor(),
            Function::Quadratic(q) => q.content_factor(),
            Function::Polynomial(p) => p.content_factor(),
        }
    }
}
