use crate::v1::{
    function::Function as FunctionEnum, linear::Term as LinearTerm, Function, Linear, Polynomial,
    Quadratic, RawSolution,
};
use anyhow::{bail, Context, Result};

/// Evaluate with a [RawSolution]
///
/// Examples
/// ---------
/// ```rust
/// # fn main() -> anyhow::Result<()> {
/// use ommx::{Evaluate, v1::{Linear, RawSolution}};
/// use maplit::hashmap;
///
/// let raw: RawSolution = hashmap! { 1 => 1.0, 2 => 2.0, 3 => 3.0 }.into();
/// // x1 + 2*x2 + 3
/// let linear = Linear::new(
///     hashmap! {
///         1 => 1.0,
///         2 => 2.0,
///     }
///     .into_iter(),
///     3.0,
/// );
/// assert_eq!(linear.evaluate(&raw)?, 1.0 * 1.0 + 2.0 * 2.0 + 3.0);
/// # Ok(()) }
/// ```
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
        let mut sum = self.constant;
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
    fn evaluate(&self, solution: &RawSolution) -> Result<f64> {
        let mut sum = if let Some(linear) = &self.linear {
            linear.evaluate(solution)?
        } else {
            0.0
        };
        for (i, j, value) in
            itertools::multizip((self.rows.iter(), self.columns.iter(), self.values.iter()))
        {
            let u = solution
                .entries
                .get(i)
                .with_context(|| format!("Variable id ({i}) is not found in the solution"))?;
            let v = solution
                .entries
                .get(j)
                .with_context(|| format!("Variable id ({j}) is not found in the solution"))?;
            sum += value * u * v;
        }
        Ok(sum)
    }
}

impl Evaluate for Polynomial {
    type Output = f64;
    fn evaluate(&self, solution: &RawSolution) -> Result<f64> {
        let mut sum = 0.0;
        for term in &self.terms {
            let mut v = term.coefficient;
            for id in &term.ids {
                v *= solution
                    .entries
                    .get(id)
                    .with_context(|| format!("Variable id ({id}) is not found in the solution"))?;
            }
            sum += v;
        }
        Ok(sum)
    }
}
