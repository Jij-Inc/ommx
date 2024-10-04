use crate::v1::{function, function::Function as FunctionEnum, Function, Linear, Quadratic};
use std::{collections::BTreeSet, iter::Sum, ops::Add};

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

impl From<f64> for Function {
    fn from(f: f64) -> Self {
        Self {
            function: Some(function::Function::Constant(f)),
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
}

impl Add for Function {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        let lhs = self.function.expect("Empty Function");
        let rhs = rhs.function.expect("Empty Function");
        match (lhs, rhs) {
            (FunctionEnum::Constant(lhs), FunctionEnum::Constant(rhs)) => Function::from(lhs + rhs),
            (FunctionEnum::Linear(lhs), FunctionEnum::Constant(rhs))
            | (FunctionEnum::Constant(rhs), FunctionEnum::Linear(lhs)) => Function::from(lhs + rhs),
            (FunctionEnum::Linear(lhs), FunctionEnum::Linear(rhs)) => Function::from(lhs + rhs),
            _ => unimplemented!(),
        }
    }
}

impl Sum for Function {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Function::from(0.0), |acc, x| acc + x)
    }
}
