use super::*;
use crate::v1;

impl From<v1::Linear> for Linear {
    fn from(linear: v1::Linear) -> Self {
        let mut new = Self::default();
        for term in linear.terms {
            new.terms.insert(term.id, term.coefficient);
        }
        new.constant = linear.constant;
        new
    }
}

impl From<Linear> for v1::Linear {
    fn from(linear: Linear) -> Self {
        let mut new = Self::default();
        for (id, coefficient) in linear.terms {
            new.terms.push(v1::linear::Term { id, coefficient });
        }
        new.constant = linear.constant;
        new
    }
}
