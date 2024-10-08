use crate::v1::{
    function::{self, Function as FunctionEnum},
    Function, Linear, Polynomial, Quadratic,
};
use std::{collections::BTreeSet, iter::*, ops::*};

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
