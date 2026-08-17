#[cfg(test)]
use crate::PolynomialBase;
use crate::{
    Coefficient, Degree, Linear, MonomialDyn, Polynomial, Quadratic, VariableID, VariableIDPair,
};
use derive_more::From;
use num::traits::Inv;
use std::borrow::Cow;

mod add;
mod approx;
mod arbitrary;
mod div;
mod evaluate;
mod evaluate_bound;
mod logical_memory;
mod mul;
mod operation;
mod parse;
mod reduce_binary_power;
mod serialize;
mod sub;
mod substitute;

pub use operation::*;

/// A real-valued function of decision variables used for objective and constraint functions.
///
/// Polynomial functions use compact coefficient maps. Other functions use a
/// validated, non-recursive reverse-Polish expression program.
#[derive(Clone, PartialEq, From, Default)]
pub enum Function {
    #[default]
    Zero,
    /// Constant term stored as a [`Coefficient`].
    Constant(Coefficient),
    Linear(Linear),
    Quadratic(Quadratic),
    Polynomial(Polynomial),
    Expression(Expression),
}

#[derive(serde::Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ExpressionRef<'a> {
    Expression { instructions: &'a [Instruction] },
}

#[derive(serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ExpressionOwned {
    Expression { instructions: Vec<Instruction> },
}

impl serde::Serialize for Function {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Function::Zero => {
                use serde::ser::SerializeMap;
                let map = serializer.serialize_map(Some(0))?;
                map.end()
            }
            Function::Constant(c) => {
                use serde::ser::SerializeMap;
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry(&(), &c.into_inner())?;
                map.end()
            }
            Function::Linear(l) => l.serialize(serializer),
            Function::Quadratic(q) => q.serialize(serializer),
            Function::Polynomial(p) => p.serialize(serializer),
            Function::Expression(expression) => ExpressionRef::Expression {
                instructions: expression.instructions(),
            }
            .serialize(serializer),
        }
    }
}

impl<'de> serde::Deserialize<'de> for Function {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Expression(ExpressionOwned),
            Polynomial(Polynomial),
        }

        match Repr::deserialize(deserializer)? {
            Repr::Polynomial(polynomial) => Ok(Function::Polynomial(polynomial).normalize()),
            Repr::Expression(ExpressionOwned::Expression { instructions }) => {
                Function::from_instructions_exact(instructions).map_err(serde::de::Error::custom)
            }
        }
    }
}

impl TryFrom<f64> for Function {
    type Error = crate::CoefficientError;
    fn try_from(value: f64) -> Result<Self, Self::Error> {
        match Coefficient::try_from(value) {
            Ok(c) => Ok(Function::Constant(c)),
            Err(crate::CoefficientError::Zero) => Ok(Function::Zero),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
impl<M> From<Result<PolynomialBase<M>, crate::CoefficientError>> for Function
where
    M: crate::Monomial,
    PolynomialBase<M>: Into<Function>,
{
    fn from(value: Result<PolynomialBase<M>, crate::CoefficientError>) -> Self {
        value.unwrap().into()
    }
}

#[doc(hidden)]
pub trait IntoFunctionForMacro {
    fn into_function_for_macro(self) -> Function;
}

impl<T> IntoFunctionForMacro for Result<T, crate::CoefficientError>
where
    Function: From<T>,
{
    fn into_function_for_macro(self) -> Function {
        Function::from(self.unwrap())
    }
}

macro_rules! impl_into_function_for_macro {
    ($($ty:ty),* $(,)?) => {
        $(
            impl IntoFunctionForMacro for $ty {
                fn into_function_for_macro(self) -> Function {
                    Function::from(self)
                }
            }
        )*
    };
}

impl_into_function_for_macro!(
    Function,
    Coefficient,
    Linear,
    Quadratic,
    Polynomial,
    crate::LinearMonomial,
    crate::QuadraticMonomial,
    MonomialDyn,
);

impl Function {
    /// Return whether this value uses the compact polynomial representation.
    pub fn is_polynomial(&self) -> bool {
        !matches!(self, Function::Expression(_))
    }

    pub fn as_polynomial(&self) -> Option<Cow<'_, Polynomial>> {
        match self {
            Function::Zero => Some(Cow::Owned(Polynomial::zero())),
            Function::Constant(c) => Some(Cow::Owned((*c).into())),
            Function::Linear(l) => Some(Cow::Owned(l.clone().into())),
            Function::Quadratic(q) => Some(Cow::Owned(q.clone().into())),
            Function::Polynomial(p) => Some(Cow::Borrowed(p)),
            Function::Expression(_) => None,
        }
    }

    pub fn constant_term(&self) -> Option<f64> {
        match self {
            Function::Zero => Some(0.0),
            Function::Constant(c) => Some(c.into_inner()),
            Function::Linear(l) => Some(l.constant_term()),
            Function::Quadratic(q) => Some(q.constant_term()),
            Function::Polynomial(p) => Some(p.constant_term()),
            Function::Expression(_) => None,
        }
    }

    pub fn linear_terms(&self) -> Option<Box<dyn Iterator<Item = (VariableID, Coefficient)> + '_>> {
        Some(match self {
            Function::Zero | Function::Constant(_) => Box::new(std::iter::empty()),
            Function::Linear(l) => Box::new(l.linear_terms()),
            Function::Quadratic(q) => Box::new(q.linear_terms()),
            Function::Polynomial(p) => Box::new(p.linear_terms()),
            Function::Expression(_) => return None,
        })
    }

    pub fn quadratic_terms(
        &self,
    ) -> Option<Box<dyn Iterator<Item = (VariableIDPair, Coefficient)> + '_>> {
        Some(match self {
            Function::Zero | Function::Constant(_) => Box::new(std::iter::empty()),
            Function::Linear(l) => Box::new(l.quadratic_terms()),
            Function::Quadratic(q) => Box::new(q.quadratic_terms()),
            Function::Polynomial(p) => Box::new(p.quadratic_terms()),
            Function::Expression(_) => return None,
        })
    }

    pub fn as_linear(&self) -> Option<Cow<'_, Linear>> {
        match self {
            Function::Zero => Some(Cow::Owned(Linear::zero())),
            Function::Constant(c) => Some(Cow::Owned((*c).into())),
            Function::Linear(l) => Some(Cow::Borrowed(l)),
            Function::Quadratic(q) => q.try_into().map(Cow::Owned).ok(),
            Function::Polynomial(p) => p.try_into().map(Cow::Owned).ok(),
            Function::Expression(_) => None,
        }
    }

    pub fn as_quadratic(&self) -> Option<Cow<'_, Quadratic>> {
        match self {
            Function::Zero => Some(Cow::Owned(Quadratic::zero())),
            Function::Constant(c) => Some(Cow::Owned((*c).into())),
            Function::Linear(l) => Some(Cow::Owned(l.clone().into())),
            Function::Quadratic(q) => Some(Cow::Borrowed(q)),
            Function::Polynomial(p) => p.try_into().map(Cow::Owned).ok(),
            Function::Expression(_) => None,
        }
    }

    pub fn num_terms(&self) -> Option<usize> {
        match self {
            Function::Zero => Some(0),
            Function::Constant(_) => Some(1),
            Function::Linear(l) => Some(l.num_terms()),
            Function::Quadratic(q) => Some(q.num_terms()),
            Function::Polynomial(p) => Some(p.num_terms()),
            Function::Expression(_) => None,
        }
    }

    pub fn degree(&self) -> Option<Degree> {
        match self {
            Function::Zero | Function::Constant(_) => Some(0.into()),
            Function::Linear(l) => Some(l.degree()),
            Function::Quadratic(q) => Some(q.degree()),
            Function::Polynomial(p) => Some(p.degree()),
            Function::Expression(_) => None,
        }
    }

    pub fn iter(&self) -> Option<Box<dyn Iterator<Item = (MonomialDyn, &Coefficient)> + '_>> {
        Some(match self {
            Function::Zero => Box::new(std::iter::empty()),
            Function::Constant(c) => Box::new(std::iter::once((MonomialDyn::default(), c))),
            Function::Linear(l) => Box::new(l.iter().map(|(k, v)| (MonomialDyn::from(*k), v))),
            Function::Quadratic(q) => Box::new(q.iter().map(|(k, v)| (MonomialDyn::from(*k), v))),
            Function::Polynomial(p) => Box::new(p.iter().map(|(k, v)| (k.clone(), v))),
            Function::Expression(_) => return None,
        })
    }

    pub fn iter_mut(
        &mut self,
    ) -> Option<Box<dyn Iterator<Item = (MonomialDyn, &mut Coefficient)> + '_>> {
        Some(match self {
            Function::Zero => Box::new(std::iter::empty()),
            Function::Constant(c) => Box::new(std::iter::once((MonomialDyn::default(), c))),
            Function::Linear(l) => Box::new(l.iter_mut().map(|(k, v)| (MonomialDyn::from(*k), v))),
            Function::Quadratic(q) => {
                Box::new(q.iter_mut().map(|(k, v)| (MonomialDyn::from(*k), v)))
            }
            Function::Polynomial(p) => Box::new(p.iter_mut().map(|(k, v)| (k.clone(), v))),
            Function::Expression(_) => return None,
        })
    }

    pub fn values(&self) -> Option<Box<dyn Iterator<Item = &Coefficient> + '_>> {
        Some(match self {
            Function::Zero => Box::new(std::iter::empty()),
            Function::Constant(c) => Box::new(std::iter::once(c)),
            Function::Linear(l) => Box::new(l.values()),
            Function::Quadratic(q) => Box::new(q.values()),
            Function::Polynomial(p) => Box::new(p.values()),
            Function::Expression(_) => return None,
        })
    }

    pub fn values_mut(&mut self) -> Option<Box<dyn Iterator<Item = &mut Coefficient> + '_>> {
        Some(match self {
            Function::Zero => Box::new(std::iter::empty()),
            Function::Constant(c) => Box::new(std::iter::once(c)),
            Function::Linear(l) => Box::new(l.values_mut()),
            Function::Quadratic(q) => Box::new(q.values_mut()),
            Function::Polynomial(p) => Box::new(p.values_mut()),
            Function::Expression(_) => return None,
        })
    }

    pub fn keys(&self) -> Option<Box<dyn Iterator<Item = MonomialDyn> + '_>> {
        Some(match self {
            Function::Zero => Box::new(std::iter::empty()),
            Function::Constant(_) => Box::new(std::iter::once(MonomialDyn::default())),
            Function::Linear(l) => Box::new(l.keys().map(|k| MonomialDyn::from(*k))),
            Function::Quadratic(q) => Box::new(q.keys().map(|k| MonomialDyn::from(*k))),
            Function::Polynomial(p) => Box::new(p.keys().cloned()),
            Function::Expression(_) => return None,
        })
    }

    /// Get a minimal positive factor `a` which make all coefficients of `a * self` integer.
    ///
    /// This returns `1` for zero function. See also <https://en.wikipedia.org/wiki/Primitive_part_and_content>.
    /// Returns an error for a composed, non-polynomial function.
    pub fn content_factor(&self) -> crate::Result<Coefficient> {
        match self {
            Function::Zero => Ok(Coefficient::one()),
            // The factor must be positive so that multiplying it preserves the sign of
            // the constraint, matching `PolynomialBase::content_factor` for Linear/Quadratic/Polynomial.
            Function::Constant(c) => Ok(c.inv()?.abs()),
            Function::Linear(l) => l.content_factor(),
            Function::Quadratic(q) => q.content_factor(),
            Function::Polynomial(p) => p.content_factor(),
            Function::Expression(_) => {
                anyhow::bail!("content_factor is only defined for polynomial functions")
            }
        }
    }
}

#[cfg(test)]
mod serde_tests {
    use super::*;

    #[test]
    fn composed_function_json_rejects_stack_underflow() {
        let serialized =
            r#"{"type":"expression","instructions":[{"instruction":"unary","value":"abs"}]}"#;
        let error = serde_json::from_str::<Function>(serialized).unwrap_err();
        assert!(error
            .to_string()
            .contains("requires 1 stack values, found 0"));
    }

    #[test]
    fn composed_function_deserialization_preserves_rpn_order() {
        let serialized = r#"{"type":"expression","instructions":[{"instruction":"push","value":{"type":"zero"}},{"instruction":"push","value":{"type":"zero"}},{"instruction":"associative","value":"mul"}]}"#;
        let function = serde_json::from_str::<Function>(serialized).unwrap();
        let Function::Expression(expression) = function else {
            panic!("deserialization must preserve an explicit expression program");
        };
        assert_eq!(
            expression.instructions(),
            &[
                Instruction::Push(Atom::Zero),
                Instruction::Push(Atom::Zero),
                Instruction::Associative(AssociativeOperator::Mul),
            ]
        );
    }

    #[test]
    fn parameterized_unary_operator_json_roundtrip() {
        let function = Function::zero().powi(-2);
        let serialized = serde_json::to_string(&function).unwrap();
        let deserialized = serde_json::from_str::<Function>(&serialized).unwrap();

        assert_eq!(deserialized, function);
    }

    #[test]
    fn composed_function_with_polynomial_and_constant_json_roundtrip() {
        let numerator = Function::from(crate::linear!(1)).abs();
        let function = (numerator / Function::try_from(2.0).unwrap()).unwrap();

        let serialized = serde_json::to_string(&function).unwrap();
        let deserialized = serde_json::from_str::<Function>(&serialized).unwrap();

        assert_eq!(deserialized, function);
    }
}
