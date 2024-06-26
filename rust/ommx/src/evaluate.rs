use crate::v1::{
    function::Function as FunctionEnum, linear::Term as LinearTerm, Constraint, Equality,
    EvaluatedConstraint, Function, Instance, Linear, Optimality, Polynomial, Quadratic, Solution,
    State,
};
use anyhow::{bail, Context, Result};
use std::collections::BTreeSet;

/// Evaluate with a [State]
pub trait Evaluate {
    type Output;
    /// Evaluate to return the output with used variable ids
    fn evaluate(&self, solution: &State) -> Result<(Self::Output, BTreeSet<u64>)>;
}

impl Evaluate for Function {
    type Output = f64;
    fn evaluate(&self, solution: &State) -> Result<(f64, BTreeSet<u64>)> {
        let out = match &self.function {
            Some(FunctionEnum::Constant(c)) => (*c, BTreeSet::new()),
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
    fn evaluate(&self, solution: &State) -> Result<(f64, BTreeSet<u64>)> {
        let mut sum = self.constant;
        let mut used_ids = BTreeSet::new();
        for LinearTerm { id, coefficient } in &self.terms {
            used_ids.insert(*id);
            let s = solution
                .entries
                .get(id)
                .with_context(|| format!("Variable id ({id}) is not found in the solution"))?;
            sum += coefficient * s;
        }
        Ok((sum, used_ids))
    }
}

impl Evaluate for Quadratic {
    type Output = f64;
    fn evaluate(&self, solution: &State) -> Result<(f64, BTreeSet<u64>)> {
        let (mut sum, mut used_ids) = if let Some(linear) = &self.linear {
            linear.evaluate(solution)?
        } else {
            (0.0, BTreeSet::new())
        };
        for (i, j, value) in
            itertools::multizip((self.rows.iter(), self.columns.iter(), self.values.iter()))
        {
            used_ids.insert(*i);
            used_ids.insert(*j);

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
        Ok((sum, used_ids))
    }
}

impl Evaluate for Polynomial {
    type Output = f64;
    fn evaluate(&self, solution: &State) -> Result<(f64, BTreeSet<u64>)> {
        let mut sum = 0.0;
        let mut used_ids = BTreeSet::new();
        for term in &self.terms {
            let mut v = term.coefficient;
            for id in &term.ids {
                used_ids.insert(*id);
                v *= solution
                    .entries
                    .get(id)
                    .with_context(|| format!("Variable id ({id}) is not found in the solution"))?;
            }
            sum += v;
        }
        Ok((sum, used_ids))
    }
}

impl Evaluate for Constraint {
    type Output = EvaluatedConstraint;

    fn evaluate(&self, solution: &State) -> Result<(Self::Output, BTreeSet<u64>)> {
        let (evaluated_value, used_ids) = self
            .function
            .as_ref()
            .context("Function is not set")?
            .evaluate(solution)?;
        let used_decision_variable_ids = used_ids.iter().cloned().collect();
        Ok((
            EvaluatedConstraint {
                id: self.id,
                equality: self.equality,
                evaluated_value,
                used_decision_variable_ids,
                name: self.name.clone(),
                parameters: self.parameters.clone(),
                description: self.description.clone(),
            },
            used_ids,
        ))
    }
}

impl Evaluate for Instance {
    type Output = Solution;

    fn evaluate(&self, state: &State) -> Result<(Self::Output, BTreeSet<u64>)> {
        let mut used_ids = BTreeSet::new();
        let mut evaluated_constraints = Vec::new();
        let mut feasible = true;
        for c in &self.constraints {
            let (c, used_ids_) = c.evaluate(state)?;
            used_ids.extend(used_ids_);
            if c.equality == Equality::EqualToZero as i32 {
                // FIXME: Add a way to specify the tolerance
                if c.evaluated_value.abs() > 1e-6 {
                    feasible = false;
                    break;
                }
            } else if c.equality == Equality::LessThanOrEqualToZero as i32 {
                if c.evaluated_value > 0.0 {
                    feasible = false;
                    break;
                }
            } else {
                bail!("Unsupported equality: {:?}", c.equality);
            }
            evaluated_constraints.push(c);
        }

        let (objective, used_ids_) = self
            .objective
            .as_ref()
            .context("Objective is not set")?
            .evaluate(state)?;
        used_ids.extend(used_ids_);
        Ok((
            Solution {
                decision_variables: self.decision_variables.clone(),
                state: Some(state.clone()),
                evaluated_constraints,
                feasible,
                objective,
                optimality: Optimality::Unspecified.into(),
            },
            used_ids,
        ))
    }
}
