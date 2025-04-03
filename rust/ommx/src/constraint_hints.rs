use crate::v1::{ConstraintHints, OneHot};
use std::collections::{HashMap, HashSet};

impl ConstraintHints {
    pub fn one_hot_constraints(&self) -> Vec<OneHot> {
        let mut result = self.one_hot_constraints.clone();
        let mut constraint_ids: HashSet<u64> = result.iter().map(|c| c.constraint_id).collect();

        if let Some(k_hot_list) = self.k_hot_constraints.get(&1) {
            for k_hot in &k_hot_list.constraints {
                if !constraint_ids.contains(&k_hot.constraint_id) {
                    constraint_ids.insert(k_hot.constraint_id);
                    let mut one_hot = OneHot::default();
                    one_hot.constraint_id = k_hot.constraint_id;
                    one_hot.decision_variables = k_hot.decision_variables.clone();
                    result.push(one_hot);
                }
            }
        }

        result
    }

    pub fn get_k_hot_constraints(&self) -> HashMap<u64, Vec<crate::v1::KHot>> {
        let mut result: HashMap<u64, Vec<crate::v1::KHot>> = HashMap::new();

        for (k, k_hot_list) in &self.k_hot_constraints {
            let mut constraints = Vec::new();
            for constraint in &k_hot_list.constraints {
                constraints.push(constraint.clone());
            }
            result.insert(*k, constraints);
        }

        let mut k1_constraint_ids: HashSet<u64> = match result.get(&1) {
            Some(k_hots) => k_hots.iter().map(|c| c.constraint_id).collect(),
            None => HashSet::new(),
        };

        let mut k1_constraints = match result.get(&1) {
            Some(k_hots) => k_hots.clone(),
            None => Vec::new(),
        };

        for one_hot in &self.one_hot_constraints {
            if !k1_constraint_ids.contains(&one_hot.constraint_id) {
                k1_constraint_ids.insert(one_hot.constraint_id);
                let mut k_hot = crate::v1::KHot::default();
                k_hot.constraint_id = one_hot.constraint_id;
                k_hot.decision_variables = one_hot.decision_variables.clone();
                k_hot.num_hot_vars = 1;
                k1_constraints.push(k_hot);
            }
        }

        if !k1_constraints.is_empty() {
            result.insert(1, k1_constraints);
        }

        result
    }
}
