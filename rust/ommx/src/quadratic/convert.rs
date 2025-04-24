use super::*;

impl Quadratic {
    pub fn add_quad_term(&mut self, ids: VariableIDPair, coefficient: Coefficient) {
        use std::collections::hash_map::Entry;
        match self.quad.entry(ids) {
            Entry::Occupied(mut entry) => {
                // May be cancelled out
                let new = *entry.get() + coefficient;
                if let Some(new) = new {
                    entry.insert(new);
                } else {
                    entry.remove();
                }
            }
            Entry::Vacant(entry) => {
                entry.insert(coefficient);
            }
        }
    }
}
