use crate::v1::{
    function::Function as FunctionEnum, linear::Term as LinearTerm, Constraint, Equality,
    EvaluatedConstraint, Function, Instance, Linear, Optimality, Polynomial, Quadratic, Relaxation,
    Solution, State,
};
use anyhow::{bail, ensure, Context, Result};
use std::collections::{BTreeMap, BTreeSet};

/// Evaluate with a [State]
pub trait Evaluate {
    type Output;
    /// Evaluate to return the output with used variable ids
    fn evaluate(&self, solution: &State) -> Result<(Self::Output, BTreeSet<u64>)>;

    /// Partially evaluate the function to return the used variable ids
    fn partial_evaluate(&mut self, state: &State) -> Result<BTreeSet<u64>>;
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

    fn partial_evaluate(&mut self, state: &State) -> Result<BTreeSet<u64>> {
        Ok(match &mut self.function {
            Some(FunctionEnum::Constant(_)) => BTreeSet::new(),
            Some(FunctionEnum::Linear(linear)) => linear.partial_evaluate(state)?,
            Some(FunctionEnum::Quadratic(quadratic)) => quadratic.partial_evaluate(state)?,
            Some(FunctionEnum::Polynomial(poly)) => poly.partial_evaluate(state)?,
            None => bail!("Function is not set"),
        })
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

    fn partial_evaluate(&mut self, state: &State) -> Result<BTreeSet<u64>> {
        let mut used = BTreeSet::new();
        let mut i = 0;
        while i < self.terms.len() {
            let LinearTerm { id, coefficient } = self.terms[i];
            if let Some(value) = state.entries.get(&id) {
                self.constant += coefficient * value;
                self.terms.swap_remove(i);
                used.insert(id);
            } else {
                i += 1;
            }
        }
        Ok(used)
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

    fn partial_evaluate(&mut self, state: &State) -> Result<BTreeSet<u64>> {
        let mut used = BTreeSet::new();
        let mut linear = BTreeMap::new();
        let mut constant = self.linear.as_ref().map_or(0.0, |l| l.constant);
        for term in self.linear.iter().flat_map(|l| l.terms.iter()) {
            if let Some(value) = state.entries.get(&term.id) {
                constant += term.coefficient * value;
                used.insert(term.id);
            } else {
                *linear.entry(term.id).or_insert(0.0) += term.coefficient;
            }
        }

        ensure!(self.rows.len() == self.columns.len());
        ensure!(self.rows.len() == self.values.len());
        let mut i = 0;
        while i < self.rows.len() {
            let (row, column, value) = (self.rows[i], self.columns[i], self.values[i]);
            match (state.entries.get(&row), state.entries.get(&column)) {
                (Some(u), Some(v)) => {
                    constant += value * u * v;
                    used.insert(row);
                    used.insert(column);
                }
                (Some(u), None) => {
                    *linear.entry(column).or_insert(0.0) += value * u;
                    used.insert(row);
                }
                (None, Some(v)) => {
                    *linear.entry(row).or_insert(0.0) += value * v;
                    used.insert(column);
                }
                _ => {
                    i += 1;
                    continue;
                }
            }
            self.rows.swap_remove(i);
            self.columns.swap_remove(i);
            self.values.swap_remove(i);
        }
        if !linear.is_empty() || constant != 0.0 {
            self.linear = Some(Linear::new(linear.into_iter(), constant));
        }
        Ok(used)
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

    fn partial_evaluate(&mut self, _state: &State) -> Result<BTreeSet<u64>> {
        todo!()
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
                subscripts: self.subscripts.clone(),
                parameters: self.parameters.clone(),
                description: self.description.clone(),
                dual_variable: None,
            },
            used_ids,
        ))
    }

    fn partial_evaluate(&mut self, _state: &State) -> Result<BTreeSet<u64>> {
        todo!()
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
                }
            } else if c.equality == Equality::LessThanOrEqualToZero as i32 {
                if c.evaluated_value > 1e-6 {
                    feasible = false;
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
                relaxation: Relaxation::Unspecified.into(),
            },
            used_ids,
        ))
    }

    fn partial_evaluate(&mut self, _state: &State) -> Result<BTreeSet<u64>> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use maplit::*;

    #[test]
    fn linear_partial_evaluate() {
        let mut linear = Linear::new([(1, 1.0), (2, 2.0), (3, 3.0), (4, 4.0)].into_iter(), 5.0);
        let state = State {
            entries: hashmap! { 1 => 1.0, 2 => 2.0, 3 => 3.0, 5 => 5.0, 6 => 6.0 },
        };
        let used = linear.partial_evaluate(&state).unwrap();
        assert_eq!(used, btreeset! { 1, 2, 3 });
        assert_eq!(linear.constant, 5.0 + 1.0 * 1.0 + 2.0 * 2.0 + 3.0 * 3.0);
        assert_eq!(linear.terms.len(), 1);
        assert_eq!(linear.terms[0].id, 4);
        assert_eq!(linear.terms[0].coefficient, 4.0);
    }
}
