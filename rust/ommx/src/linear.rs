use crate::v1::{linear::Term, Linear};

impl Linear {
    pub fn new(terms: impl Iterator<Item = (u64, f64)>, constant: f64) -> Self {
        Self {
            terms: terms
                .map(|(id, coefficient)| Term { id, coefficient })
                .collect(),
            constant,
        }
    }
}
