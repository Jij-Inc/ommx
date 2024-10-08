use crate::v1::{Linear, Quadratic};
use std::{
    collections::{BTreeMap, BTreeSet},
    ops::Add,
};

impl Quadratic {
    pub fn used_decision_variable_ids(&self) -> BTreeSet<u64> {
        self.columns
            .iter()
            .chain(self.rows.iter())
            .cloned()
            .collect()
    }

    fn quad_iter(&self) -> impl Iterator<Item = ((u64, u64), f64)> + '_ {
        self.columns
            .iter()
            .zip(self.rows.iter())
            .zip(self.values.iter())
            .map(|((column, row), value)| ((*column, *row), *value))
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

impl FromIterator<((u64, u64), f64)> for Quadratic {
    fn from_iter<I: IntoIterator<Item = ((u64, u64), f64)>>(iter: I) -> Self {
        let mut columns = Vec::new();
        let mut rows = Vec::new();
        let mut values = Vec::new();
        for ((column, row), value) in iter {
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

impl IntoIterator for Quadratic {
    type Item = (Vec<u64>, f64);
    type IntoIter = Box<dyn Iterator<Item = Self::Item>>;

    fn into_iter(self) -> Self::IntoIter {
        assert_eq!(self.columns.len(), self.rows.len());
        assert_eq!(self.columns.len(), self.values.len());
        let n = self.columns.len();
        let quad = (0..n).map(move |i| (vec![self.columns[i], self.rows[i]], self.values[i]));
        if let Some(linear) = self.linear {
            Box::new(
                quad.chain(
                    linear
                        .into_iter()
                        .map(|(id, c)| (id.into_iter().collect(), c)),
                ),
            )
        } else {
            Box::new(quad)
        }
    }
}

impl Add for Quadratic {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        let mut map: BTreeMap<(u64, u64), f64> = self.quad_iter().collect();
        for (id, value) in rhs.quad_iter() {
            *map.entry(id).or_default() += value;
        }
        let mut out: Self = map.into_iter().collect();
        out.linear = match (self.linear, rhs.linear) {
            (Some(l), Some(r)) => Some(l + r),
            (Some(l), None) => Some(l),
            (None, Some(r)) => Some(r),
            (None, None) => None,
        };
        out
    }
}

impl Add<Linear> for Quadratic {
    type Output = Self;

    fn add(mut self, rhs: Linear) -> Self {
        if let Some(linear) = self.linear {
            self.linear = Some(linear + rhs);
        } else {
            self.linear = Some(rhs);
        }
        self
    }
}

impl Add<f64> for Quadratic {
    type Output = Self;

    fn add(mut self, rhs: f64) -> Self {
        if let Some(linear) = self.linear {
            self.linear = Some(linear + rhs);
        } else {
            self.linear = Some(rhs.into());
        }
        self
    }
}

impl_add_inverse!(Linear, Quadratic);
impl_add_inverse!(f64, Quadratic);
