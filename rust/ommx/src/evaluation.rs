use crate::v1::{
    function::Function as FunctionEnum, linear::Term as LinearTerm, Function, Linear, Polynomial,
    Quadratic, RawSolution,
};
use anyhow::{bail, Context, Result};

/// Evaluate with a [RawSolution]
pub trait Evaluate {
    type Output;
    fn evaluate(&self, solution: &RawSolution) -> Result<Self::Output>;
}

impl Evaluate for Function {
    type Output = f64;
    fn evaluate(&self, solution: &RawSolution) -> Result<f64> {
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
    fn evaluate(&self, solution: &RawSolution) -> Result<f64> {
        let mut sum = 0.0;
        for LinearTerm { id, coefficient } in &self.terms {
            let s = solution
                .entries
                .get(id)
                .with_context(|| format!("Variable id ({id}) is not found in the solution"))?;
            sum += coefficient * s;
        }
        Ok(sum)
    }
}

impl Evaluate for Quadratic {
    type Output = f64;
    fn evaluate(&self, _solution: &RawSolution) -> Result<f64> {
        todo!()
    }
}

impl Evaluate for Polynomial {
    type Output = f64;
    fn evaluate(&self, _solution: &RawSolution) -> Result<f64> {
        todo!()
    }
}
