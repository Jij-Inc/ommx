use super::*;
use crate::v1;

impl Linear {
    pub fn new(terms: HashMap<VariableID, Coefficient>, constant: Offset) -> Self {
        Self { terms, constant }
    }

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

    pub fn add_constant(&mut self, constant: Offset) {
        self.constant += constant;
    }

    pub fn linear_terms(&self) -> impl Iterator<Item = (VariableID, Coefficient)> + '_ {
        self.terms
            .iter()
            .map(|(id, coefficient)| (*id, *coefficient))
    }
}

impl From<Offset> for Linear {
    fn from(constant: Offset) -> Self {
        Self {
            terms: HashMap::new(),
            constant,
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

impl<'a> IntoIterator for &'a Linear {
    type Item = (Option<VariableID>, Coefficient);
    type IntoIter = Box<dyn Iterator<Item = Self::Item> + 'a>;

    fn into_iter(self) -> Self::IntoIter {
        if let Ok(constant) = self.constant.try_into() {
            Box::new(
                self.linear_terms()
                    .map(|(id, coefficient)| (Some(id), coefficient))
                    .chain(std::iter::once((None, constant))),
            )
        } else {
            Box::new(
                self.linear_terms()
                    .map(|(id, coefficient)| (Some(id), coefficient)),
            )
        }
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
