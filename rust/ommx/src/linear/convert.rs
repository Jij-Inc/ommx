use super::*;
use crate::v1;

impl Linear {
    pub fn add_term(&mut self, id: VariableID, coefficient: Coefficient) {
        use std::collections::hash_map::Entry;
        match self.terms.entry(id) {
            Entry::Occupied(mut entry) => {
                *entry.get_mut() += coefficient;
            }
            Entry::Vacant(entry) => {
                entry.insert(coefficient);
            }
        }
    }
}

impl From<Linear> for v1::Linear {
    fn from(linear: Linear) -> Self {
        let mut new = Self::default();
        for (id, coefficient) in linear.terms {
            new.terms.push(v1::linear::Term {
                id: id.into(),
                coefficient: coefficient.into(),
            });
        }
        new.constant = linear.constant.into();
        new
    }
}
