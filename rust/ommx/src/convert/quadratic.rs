use crate::v1::{Linear, Quadratic};
use std::collections::BTreeSet;

impl Quadratic {
    pub fn used_decision_variable_ids(&self) -> BTreeSet<u64> {
        self.columns
            .iter()
            .chain(self.rows.iter())
            .cloned()
            .collect()
    }
}

impl From<f64> for Quadratic {
    fn from(c: f64) -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            values: Vec::new(),
            linear: Some(c.into()),
        }
    }
}

impl From<Linear> for Quadratic {
    fn from(l: Linear) -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            values: Vec::new(),
            linear: Some(l),
        }
    }
}

impl FromIterator<(u64, u64, f64)> for Quadratic {
    fn from_iter<I: IntoIterator<Item = (u64, u64, f64)>>(iter: I) -> Self {
        let mut columns = Vec::new();
        let mut rows = Vec::new();
        let mut values = Vec::new();
        for (column, row, value) in iter {
            columns.push(column);
            rows.push(row);
            values.push(value);
        }
        Self {
            columns,
            rows,
            values,
            linear: None,
        }
    }
}
