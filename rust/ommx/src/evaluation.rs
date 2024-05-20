use crate::v1::{
    function::Function as FunctionEnum, Function, Linear, Polynomial, Quadratic, Solution,
};
use anyhow::{bail, Result};

/// Evaluate with the given solution.
pub trait Evaluate {
    type Output;
    fn evaluate(&self, solution: &Solution) -> Result<Self::Output>;
}

impl Evaluate for Function {
    type Output = f64;
    fn evaluate(&self, solution: &Solution) -> Result<f64> {
        let out = match &self.function {
            Some(FunctionEnum::Constant(c)) => *c,
            Some(FunctionEnum::Linear(linear)) => linear.evaluate(solution)?,
            Some(FunctionEnum::Quadratic(quadratic)) => quadratic.evaluate(solution)?,
            Some(FunctionEnum::Polynomial(poly)) => poly.evaluate(solution)?,
            None => bail!("Function is not set"),
        };
        Ok(out)
    }
}

impl Evaluate for Linear {
    type Output = f64;
    fn evaluate(&self, solution: &Solution) -> Result<f64> {
        todo!()
    }
}

impl Evaluate for Quadratic {
    type Output = f64;
    fn evaluate(&self, solution: &Solution) -> Result<f64> {
        todo!()
    }
}

impl Evaluate for Polynomial {
    type Output = f64;
    fn evaluate(&self, solution: &Solution) -> Result<f64> {
        todo!()
    }
}
