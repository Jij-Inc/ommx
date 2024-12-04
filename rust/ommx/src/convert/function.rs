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

impl<'a> IntoIterator for &'a Function {
    type Item = (Vec<u64>, f64);
    type IntoIter = Box<dyn Iterator<Item = Self::Item> + 'a>;

    fn into_iter(self) -> Self::IntoIter {
        match &self.function {
            Some(FunctionEnum::Constant(c)) => Box::new(std::iter::once((Vec::new(), *c))),
            Some(FunctionEnum::Linear(linear)) => Box::new(
                linear
                    .into_iter()
                    .map(|(id, c)| (id.into_iter().collect(), c)),
            ),
            Some(FunctionEnum::Quadratic(quad)) => Box::new(quad.into_iter()),
            Some(FunctionEnum::Polynomial(poly)) => Box::new(poly.into_iter()),
            None => Box::new(std::iter::empty()),
        }
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

    pub fn degree(&self) -> u32 {
        match &self.function {
            Some(FunctionEnum::Constant(_)) => 0,
            Some(FunctionEnum::Linear(linear)) => linear.degree(),
            Some(FunctionEnum::Quadratic(quad)) => quad.degree(),
            Some(FunctionEnum::Polynomial(poly)) => poly.degree(),
            None => 0,
        }
    }

    pub fn as_linear(self) -> Option<Linear> {
        match self.function? {
            FunctionEnum::Constant(c) => Some(Linear::from(c)),
            FunctionEnum::Linear(linear) => Some(linear),
            FunctionEnum::Quadratic(quadratic) => quadratic.as_linear(),
            FunctionEnum::Polynomial(poly) => poly.as_linear(),
        }
    }

    pub fn as_constant(self) -> Option<f64> {
        match self.function? {
            FunctionEnum::Constant(c) => Some(c),
            FunctionEnum::Linear(linear) => linear.as_constant(),
            FunctionEnum::Quadratic(quadratic) => quadratic.as_constant(),
            FunctionEnum::Polynomial(poly) => poly.as_constant(),
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
    type Parameters = (usize, u32, u64);
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with((num_terms, max_degree, max_id): Self::Parameters) -> Self::Strategy {
        let linear = if max_degree >= 1 {
            Linear::arbitrary_with((num_terms, max_id))
        } else {
            super::arbitrary_coefficient()
                .prop_map(Linear::from)
                .boxed()
        };
        let quad = if max_degree >= 2 {
            Quadratic::arbitrary_with((num_terms, max_id))
        } else {
            linear.clone().prop_map(Quadratic::from).boxed()
        };
        prop_oneof![
            super::arbitrary_coefficient().prop_map(Function::from),
            linear.prop_map(Function::from),
            quad.prop_map(Function::from),
            Polynomial::arbitrary_with((num_terms, max_degree, max_id)).prop_map(Function::from),
        ]
        .boxed()
    }

    fn arbitrary() -> Self::Strategy {
        (0..10_usize, 0..5_u32, 0..10_u64)
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

    proptest! {
        #[test]
        fn test_as_linear_roundtrip(f in Function::arbitrary_with((5, 1, 10))) {
            let linear = f.clone().as_linear().unwrap();
            // `Function::Constant(c)` and `Function::Linear(Linear { terms: [], constant: c })` are mathematically same, but not structurally same.
            prop_assert!(f.abs_diff_eq(&Function::from(linear), 1e-10));
        }

        #[test]
        fn test_as_constant_roundtrip(f in Function::arbitrary_with((5, 0, 10))) {
            let c = f.clone().as_constant().unwrap();
            prop_assert!(f.abs_diff_eq(&Function::from(c), 1e-10));
        }

        #[test]
        fn test_max_degree_0(f in Function::arbitrary_with((5, 0, 10))) {
            prop_assert!(f.degree() == 0);
        }

        #[test]
        fn test_max_degree_1(f in Function::arbitrary_with((5, 1, 10))) {
            prop_assert!(f.degree() <= 1);
        }

        #[test]
        fn test_max_degree_2(f in Function::arbitrary_with((5, 2, 10))) {
            prop_assert!(f.degree() <= 2);
        }

        #[test]
        fn test_as_linear_any(f in Function::arbitrary()) {
            prop_assert!((dbg!(f.degree()) >= 2) ^ dbg!(f.as_linear()).is_some());
        }

        #[test]
        fn test_as_const_any(f in Function::arbitrary()) {
            prop_assert!((dbg!(f.degree()) >= 1) ^ dbg!(f.as_constant()).is_some());
        }
    }
}
