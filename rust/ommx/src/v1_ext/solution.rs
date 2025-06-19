use crate::v1::Solution;
use std::collections::BTreeSet;

impl Solution {
    pub fn decision_variable_ids(&self) -> BTreeSet<u64> {
        self.decision_variables.iter().map(|v| v.id).collect()
    }

    pub fn constraint_ids(&self) -> BTreeSet<u64> {
        self.evaluated_constraints.iter().map(|c| c.id).collect()
    }

    pub fn get_feasible_unrelaxed(&self) -> bool {
        match self.feasible_relaxed {
            Some(_) => self.feasible,
            None =>
            {
                #[allow(deprecated)]
                self.feasible_unrelaxed
            }
        }
    }

    pub fn get_feasible_relaxed(&self) -> bool {
        match self.feasible_relaxed {
            Some(feasible) => feasible,
            None => self.feasible,
        }
    }

    pub fn get_feasible(&self) -> bool {
        self.get_feasible_unrelaxed()
    }
}
