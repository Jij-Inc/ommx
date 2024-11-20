use crate::v1::{
    function::{self, Function as FunctionEnum},
    Function, Linear, Polynomial, Quadratic,
};
use approx::AbsDiffEq;
use num::Zero;
use proptest::prelude::*;
use std::{collections::BTreeSet, fmt, iter::*, ops::*};

impl Zero for Function {
    fn zero() -> Self {
        Self {
            function: Some(function::Function::Constant(0.0)),
        }
    }

    fn is_zero(&self) -> bool {
        match &self.function {
            Some(FunctionEnum::Constant(c)) => c.is_zero(),
            Some(FunctionEnum::Linear(linear)) => linear.is_zero(),
            Some(FunctionEnum::Quadratic(quadratic)) => quadratic.is_zero(),
            Some(FunctionEnum::Polynomial(poly)) => poly.is_zero(),
            _ => false,
        }
    }
}

impl From<function::Function> for Function {
    fn from(f: function::Function) -> Self {
        Self { function: Some(f) }
    }
}

impl From<Linear> for Function {
    fn from(linear: Linear) -> Self {
        Self {
            function: Some(function::Function::Linear(linear)),
        }
    }
}

impl From<Quadratic> for Function {
    fn from(q: Quadratic) -> Self {
        Self {
            function: Some(function::Function::Quadratic(q)),
        }
    }
}

impl From<Polynomial> for Function {
    fn from(poly: Polynomial) -> Self {
        Self {
            function: Some(function::Function::Polynomial(poly)),
        }
    }
}

impl From<f64> for Function {
    fn from(f: f64) -> Self {
        Self {
            function: Some(function::Function::Constant(f)),
        }
    }
}

impl FromIterator<(u64, f64)> for Function {
    fn from_iter<I: IntoIterator<Item = (u64, f64)>>(iter: I) -> Self {
        let linear: Linear = iter.into_iter().collect();
        linear.into()
    }
}

impl FromIterator<((u64, u64), f64)> for Function {
    fn from_iter<I: IntoIterator<Item = ((u64, u64), f64)>>(iter: I) -> Self {
        let quad: Quadratic = iter.into_iter().collect();
        quad.into()
    }
}

impl FromIterator<(Vec<u64>, f64)> for Function {
    fn from_iter<I: IntoIterator<Item = (Vec<u64>, f64)>>(iter: I) -> Self {
        let poly: Polynomial = iter.into_iter().collect();
        poly.into()
    }
}

impl Function {
    pub fn used_decision_variable_ids(&self) -> BTreeSet<u64> {
        match &self.function {
            Some(FunctionEnum::Linear(linear)) => linear.used_decision_variable_ids(),
            Some(FunctionEnum::Quadratic(quadratic)) => quadratic.used_decision_variable_ids(),
            Some(FunctionEnum::Polynomial(poly)) => poly.used_decision_variable_ids(),
            _ => BTreeSet::new(),
        }
    }
}

impl Add for Function {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        let lhs = self.function.expect("Empty Function");
        let rhs = rhs.function.expect("Empty Function");
        match (lhs, rhs) {
            (FunctionEnum::Constant(lhs), FunctionEnum::Constant(rhs)) => Function::from(lhs + rhs),
            // Linear output
            (FunctionEnum::Linear(lhs), FunctionEnum::Constant(rhs))
            | (FunctionEnum::Constant(rhs), FunctionEnum::Linear(lhs)) => Function::from(lhs + rhs),
            (FunctionEnum::Linear(lhs), FunctionEnum::Linear(rhs)) => Function::from(lhs + rhs),
            // Quadratic output
            (FunctionEnum::Quadratic(lhs), FunctionEnum::Constant(rhs))
            | (FunctionEnum::Constant(rhs), FunctionEnum::Quadratic(lhs)) => {
                Function::from(lhs + rhs)
            }
            (FunctionEnum::Quadratic(lhs), FunctionEnum::Linear(rhs))
            | (FunctionEnum::Linear(rhs), FunctionEnum::Quadratic(lhs)) => {
                Function::from(lhs + rhs)
            }
            (FunctionEnum::Quadratic(lhs), FunctionEnum::Quadratic(rhs)) => {
                Function::from(lhs + rhs)
            }
            // Polynomial output
            (FunctionEnum::Polynomial(lhs), FunctionEnum::Constant(rhs))
            | (FunctionEnum::Constant(rhs), FunctionEnum::Polynomial(lhs)) => {
                Function::from(lhs + rhs)
            }
            (FunctionEnum::Polynomial(lhs), FunctionEnum::Linear(rhs))
            | (FunctionEnum::Linear(rhs), FunctionEnum::Polynomial(lhs)) => {
                Function::from(lhs + rhs)
            }
            (FunctionEnum::Polynomial(lhs), FunctionEnum::Quadratic(rhs))
            | (FunctionEnum::Quadratic(rhs), FunctionEnum::Polynomial(lhs)) => {
                Function::from(lhs + rhs)
            }
            (FunctionEnum::Polynomial(lhs), FunctionEnum::Polynomial(rhs)) => {
                Function::from(lhs + rhs)
            }
        }
    }
}

impl_add_from!(Function, f64);
impl_add_from!(Function, Linear);
impl_add_from!(Function, Quadratic);
impl_add_from!(Function, Polynomial);
impl_add_inverse!(f64, Function);
impl_add_inverse!(Linear, Function);
impl_add_inverse!(Quadratic, Function);
impl_add_inverse!(Polynomial, Function);
impl_sub_by_neg_add!(Function, Function);
impl_sub_by_neg_add!(Function, f64);
impl_sub_by_neg_add!(Function, Linear);
impl_sub_by_neg_add!(Function, Quadratic);
impl_sub_by_neg_add!(Function, Polynomial);

impl Mul for Function {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self {
        let lhs = self.function.expect("Empty Function");
        let rhs = rhs.function.expect("Empty Function");
        match (lhs, rhs) {
            (FunctionEnum::Constant(lhs), FunctionEnum::Constant(rhs)) => Function::from(lhs * rhs),
            (FunctionEnum::Linear(lhs), FunctionEnum::Constant(rhs))
            | (FunctionEnum::Constant(rhs), FunctionEnum::Linear(lhs)) => Function::from(lhs * rhs),
            (FunctionEnum::Linear(lhs), FunctionEnum::Linear(rhs)) => Function::from(lhs * rhs),
            (FunctionEnum::Quadratic(lhs), FunctionEnum::Constant(rhs))
            | (FunctionEnum::Constant(rhs), FunctionEnum::Quadratic(lhs)) => {
                Function::from(lhs * rhs)
            }
            (FunctionEnum::Quadratic(lhs), FunctionEnum::Linear(rhs))
            | (FunctionEnum::Linear(rhs), FunctionEnum::Quadratic(lhs)) => {
                Function::from(lhs * rhs)
            }
            (FunctionEnum::Quadratic(lhs), FunctionEnum::Quadratic(rhs)) => {
                Function::from(lhs * rhs)
            }
            (FunctionEnum::Polynomial(lhs), FunctionEnum::Constant(rhs))
            | (FunctionEnum::Constant(rhs), FunctionEnum::Polynomial(lhs)) => {
                Function::from(lhs * rhs)
            }
            (FunctionEnum::Polynomial(lhs), FunctionEnum::Linear(rhs))
            | (FunctionEnum::Linear(rhs), FunctionEnum::Polynomial(lhs)) => {
                Function::from(lhs * rhs)
            }
            (FunctionEnum::Polynomial(lhs), FunctionEnum::Quadratic(rhs))
            | (FunctionEnum::Quadratic(rhs), FunctionEnum::Polynomial(lhs)) => {
                Function::from(lhs * rhs)
            }
            (FunctionEnum::Polynomial(lhs), FunctionEnum::Polynomial(rhs)) => {
                Function::from(lhs * rhs)
            }
        }
    }
}

impl_neg_by_mul!(Function);
impl_mul_from!(Function, f64, Function);
impl_mul_from!(Function, Linear, Function);
impl_mul_from!(Function, Quadratic, Function);
impl_mul_from!(Function, Polynomial, Function);
impl_mul_inverse!(f64, Function);
impl_mul_inverse!(Linear, Function);
impl_mul_inverse!(Quadratic, Function);
impl_mul_inverse!(Polynomial, Function);

impl Sum for Function {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Function::from(0.0), |acc, x| acc + x)
    }
}

impl Product for Function {
    fn product<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Function::from(1.0), |acc, x| acc * x)
    }
}

impl Arbitrary for Function {
    type Parameters = (usize, usize, u64);
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with((num_terms, max_degree, max_id): Self::Parameters) -> Self::Strategy {
        prop_oneof![
            prop_oneof![Just(0.0), -1.0..1.0_f64].prop_map(Function::from),
            Linear::arbitrary_with((num_terms, max_id)).prop_map(Function::from),
            Quadratic::arbitrary_with((num_terms, max_id)).prop_map(Function::from),
            Polynomial::arbitrary_with((num_terms, max_degree, max_id)).prop_map(Function::from),
        ]
        .boxed()
    }

    fn arbitrary() -> Self::Strategy {
        (0..10_usize, 0..5_usize, 0..10_u64)
            .prop_flat_map(Self::arbitrary_with)
            .boxed()
    }
}

impl AbsDiffEq for Function {
    type Epsilon = f64;

    fn default_epsilon() -> Self::Epsilon {
        f64::default_epsilon()
    }

    fn abs_diff_eq(&self, other: &Self, epsilon: Self::Epsilon) -> bool {
        let lhs = self.function.as_ref().expect("Empty Function");
        let rhs = other.function.as_ref().expect("Empty Function");
        match (lhs, rhs) {
            // Same order
            (FunctionEnum::Constant(lhs), FunctionEnum::Constant(rhs)) => {
                lhs.abs_diff_eq(rhs, epsilon)
            }
            (FunctionEnum::Linear(lhs), FunctionEnum::Linear(rhs)) => lhs.abs_diff_eq(rhs, epsilon),
            (FunctionEnum::Quadratic(lhs), FunctionEnum::Quadratic(rhs)) => {
                lhs.abs_diff_eq(rhs, epsilon)
            }
            (FunctionEnum::Polynomial(lhs), FunctionEnum::Polynomial(rhs)) => {
                lhs.abs_diff_eq(rhs, epsilon)
            }
            // Upcast to higher order
            (FunctionEnum::Constant(lhs), FunctionEnum::Linear(rhs))
            | (FunctionEnum::Linear(rhs), FunctionEnum::Constant(lhs)) => {
                let lhs = Linear::from(*lhs);
                lhs.abs_diff_eq(rhs, epsilon)
            }
            (FunctionEnum::Constant(lhs), FunctionEnum::Quadratic(rhs))
            | (FunctionEnum::Quadratic(rhs), FunctionEnum::Constant(lhs)) => {
                let lhs = Quadratic::from(*lhs);
                lhs.abs_diff_eq(rhs, epsilon)
            }
            (FunctionEnum::Constant(lhs), FunctionEnum::Polynomial(rhs))
            | (FunctionEnum::Polynomial(rhs), FunctionEnum::Constant(lhs)) => {
                let lhs = Polynomial::from(*lhs);
                lhs.abs_diff_eq(rhs, epsilon)
            }
            (FunctionEnum::Linear(lhs), FunctionEnum::Quadratic(rhs))
            | (FunctionEnum::Quadratic(rhs), FunctionEnum::Linear(lhs)) => {
                let lhs = Quadratic::from(lhs.clone());
                lhs.abs_diff_eq(rhs, epsilon)
            }
            (FunctionEnum::Linear(lhs), FunctionEnum::Polynomial(rhs))
            | (FunctionEnum::Polynomial(rhs), FunctionEnum::Linear(lhs)) => {
                let lhs = Polynomial::from(lhs.clone());
                lhs.abs_diff_eq(rhs, epsilon)
            }
            (FunctionEnum::Quadratic(lhs), FunctionEnum::Polynomial(rhs))
            | (FunctionEnum::Polynomial(rhs), FunctionEnum::Quadratic(lhs)) => {
                let lhs = Polynomial::from(lhs.clone());
                lhs.abs_diff_eq(rhs, epsilon)
            }
        }
    }
}

impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.function {
            Some(FunctionEnum::Constant(c)) => write!(f, "{}", c),
            Some(FunctionEnum::Linear(linear)) => write!(f, "{}", linear),
            Some(FunctionEnum::Quadratic(quadratic)) => write!(f, "{}", quadratic),
            Some(FunctionEnum::Polynomial(poly)) => write!(f, "{}", poly),
            None => write!(f, "0"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    test_algebraic!(Function);
}
