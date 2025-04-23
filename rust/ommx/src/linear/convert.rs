use super::*;
use crate::v1;

impl Linear {
    pub fn add_term(&mut self, id: VariableID, coefficient: Coefficient) {
        use std::collections::hash_map::Entry;
        match self.terms.entry(id) {
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

impl FromIterator<(VariableID, Coefficient)> for Linear {
    fn from_iter<I: IntoIterator<Item = (VariableID, Coefficient)>>(iter: I) -> Self {
        let mut out = Linear::default();
        for (id, coefficient) in iter {
            out.add_term(id, coefficient);
        }
        out
    }
}

impl FromIterator<(Option<VariableID>, Coefficient)> for Linear {
    fn from_iter<I: IntoIterator<Item = (Option<VariableID>, Coefficient)>>(iter: I) -> Self {
        let mut out = Linear::default();
        for (id, coefficient) in iter {
            if let Some(id) = id {
                out.add_term(id, coefficient);
            } else {
                out.constant += coefficient.into();
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_term_cancel_out() {
        let mut linear = Linear::default();
        linear.add_term(1.into(), 1.0.try_into().unwrap());
        linear.add_term(1.into(), (-1.0).try_into().unwrap());
        assert_eq!(linear.terms.len(), 0);
    }
}
