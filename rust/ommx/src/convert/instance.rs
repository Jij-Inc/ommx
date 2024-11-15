use crate::v1::{decision_variable::Kind, Equality, Instance};
use anyhow::{bail, Context, Result};
use std::collections::{BTreeMap, BTreeSet};

impl Instance {
    fn binary_variables(&self) -> BTreeSet<u64> {
        self.decision_variables
            .iter()
            .filter_map(|v| {
                if v.kind == Kind::Binary as i32 {
                    Some(v.id)
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn to_pubo(&self) -> Result<BTreeMap<Vec<u64>, f64>> {
        let binary = self.binary_variables();

        let objective = self
            .objective
            .as_ref()
            .context("No objective found in the instance")?;

        if !objective.used_decision_variable_ids().is_subset(&binary) {
            bail!("Objective contains non-binary variables");
        }

        let mut pubo: BTreeMap<Vec<u64>, f64> = objective
            .into_iter()
            .map(|(mut id, coefficient)| {
                // Order reduction for binary variable by x^2 = x
                id.dedup();
                (id, coefficient)
            })
            .collect();
        for constraint in &self.constraints {
            if constraint.equality != Equality::EqualToZero as i32 {
                bail!("Only equality constraints are supported");
            }
            let f = constraint
                .function
                .as_ref()
                .context("No function found in the constraint")?;
            if !f.used_decision_variable_ids().is_subset(&binary) {
                bail!("Constraint contains non-binary variables");
            }
            if let Some(dual_variable) = constraint.dual_variable {
                if dual_variable != 0.0 {
                    for (mut id, coefficient) in f.into_iter() {
                        id.dedup();
                        let coefficient = dual_variable * coefficient;
                        pubo.entry(id)
                            .and_modify(|c| *c += coefficient)
                            .or_insert(coefficient);
                    }
                }
            }
            if let Some(penalty_weight) = constraint.penalty_weight {
                if penalty_weight != 0.0 {
                    let f2 = f.clone() * f.clone();
                    for (mut id, coefficient) in f2.into_iter() {
                        id.dedup();
                        let coefficient = penalty_weight * coefficient;
                        pubo.entry(id)
                            .and_modify(|c| *c += coefficient)
                            .or_insert(coefficient);
                    }
                }
            }
        }

        Ok(pubo)
    }

    pub fn to_qubo(&self) -> Result<Qubo> {
        Qubo::from_pubo(self.to_pubo()?)
    }
}

/// Quadratic unconstrained binary optimization (QUBO)
#[derive(Debug, Clone)]
pub struct Qubo {
    pub qubo: BTreeMap<(u64, u64), f64>,
    pub constant: f64,
}

impl Qubo {
    pub fn from_pubo(pubo: BTreeMap<Vec<u64>, f64>) -> Result<Self> {
        let mut constant = 0.0;
        let mut qubo = BTreeMap::new();
        for (id, coefficient) in pubo {
            match id[..] {
                [a, b] => {
                    qubo.insert((a, b), coefficient);
                }
                [a] => {
                    qubo.insert((a, a), coefficient);
                }
                [] => {
                    constant += coefficient;
                }
                _ => bail!("Higher order terms are not supported: {id:?}"),
            }
        }
        Ok(Self { qubo, constant })
    }
}
