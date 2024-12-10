use crate::v1::Solution;
use std::collections::BTreeSet;

impl Solution {
    pub fn decision_variable_ids(&self) -> BTreeSet<u64> {
        self.decision_variables.iter().map(|v| v.id).collect()
    }

    pub fn constraint_ids(&self) -> BTreeSet<u64> {
        self.evaluated_constraints.iter().map(|c| c.id).collect()
    }
}
