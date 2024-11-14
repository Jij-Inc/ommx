use crate::v1::{decision_variable::Kind, Instance};
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

        let objective_pubo: BTreeMap<Vec<u64>, f64> = objective
            .into_iter()
            .map(|(mut id, coefficient)| {
                // Order reduction for binary variable by x^2 = x
                id.dedup();
                (id, coefficient)
            })
            .collect();

        Ok(objective_pubo)
    }
}
