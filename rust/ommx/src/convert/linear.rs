use crate::v1::{linear::Term, Linear, Quadratic};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    iter::Sum,
    ops::*,
};

impl Linear {
    pub fn new(terms: impl Iterator<Item = (u64, f64)>, constant: f64) -> Self {
        Self {
            terms: terms
                .map(|(id, coefficient)| Term { id, coefficient })
                .collect(),
            constant,
        }
    }

    pub fn single_term(id: u64, coefficient: f64) -> Self {
        Self {
            terms: vec![Term { id, coefficient }],
            constant: 0.0,
        }
    }

    pub fn used_decision_variable_ids(&self) -> BTreeSet<u64> {
        self.terms.iter().map(|term| term.id).collect()
    }
}

impl From<u64> for Linear {
    fn from(id: u64) -> Self {
        Self::single_term(id, 1.0)
    }
}

impl From<f64> for Linear {
    fn from(constant: f64) -> Self {
        Self {
            terms: vec![],
            constant,
        }
    }
}

impl Add for Linear {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        let mut terms = HashMap::new();
        for term in self.terms.iter().chain(rhs.terms.iter()) {
            *terms.entry(term.id).or_default() += term.coefficient;
        }
        Self {
            terms: terms
                .into_iter()
                .map(|(id, coefficient)| Term { id, coefficient })
                .collect(),
            constant: self.constant + rhs.constant,
        }
    }
}

impl Add<f64> for Linear {
    type Output = Self;

    fn add(self, rhs: f64) -> Self {
        Self {
            terms: self.terms,
            constant: self.constant + rhs,
        }
    }
}

impl Add<Linear> for f64 {
    type Output = Linear;

    fn add(self, rhs: Linear) -> Linear {
        rhs + self
    }
}

impl Sum for Linear {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Linear::from(0), Add::add)
    }
}

impl Mul<f64> for Linear {
    type Output = Self;

    fn mul(self, rhs: f64) -> Self {
        Self {
            terms: self
                .terms
                .into_iter()
                .map(|term| Term {
                    id: term.id,
                    coefficient: term.coefficient * rhs,
                })
                .collect(),
            constant: self.constant * rhs,
        }
    }
}

impl Mul<Linear> for f64 {
    type Output = Linear;

    fn mul(self, rhs: Linear) -> Linear {
        rhs * self
    }
}

impl Mul for Linear {
    type Output = Quadratic;

    fn mul(self, rhs: Self) -> Quadratic {
        let mut terms = BTreeMap::new();
        for a in &self.terms {
            for b in &rhs.terms {
                let (row, col) = if a.id < b.id {
                    (a.id, b.id)
                } else {
                    (b.id, a.id)
                };
                *terms.entry((row, col)).or_default() += a.coefficient * b.coefficient;
            }
        }
        let mut columns = Vec::new();
        let mut rows = Vec::new();
        let mut values = Vec::new();
        for ((row, col), value) in terms {
            columns.push(col);
            rows.push(row);
            values.push(value);
        }
        let c = self.constant;
        Quadratic {
            columns,
            rows,
            values,
            linear: Some(self * rhs.constant + c * rhs),
        }
    }
}
