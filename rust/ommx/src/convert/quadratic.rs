use crate::v1::{Linear, Polynomial, Quadratic};
use approx::AbsDiffEq;
use num::Zero;
use proptest::prelude::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    ops::{Add, Mul},
};

impl Zero for Quadratic {
    fn zero() -> Self {
        Self {
            columns: vec![],
            rows: vec![],
            values: vec![],
            linear: Some(Linear::zero()),
        }
    }

    fn is_zero(&self) -> bool {
        self.columns.is_empty()
            && self.rows.is_empty()
            && self.values.is_empty()
            && self.linear.as_ref().map_or(true, |l| l.is_zero())
    }
}

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
        let mut terms = BTreeMap::new();
        for ((row, col), value) in iter {
            let id = if row < col { (row, col) } else { (col, row) };
            *terms.entry(id).or_default() += value;
        }
        let mut columns = Vec::new();
        let mut rows = Vec::new();
        let mut values = Vec::new();
        for ((row, col), value) in terms {
            columns.push(col);
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
            let v = map.entry(id).or_default();
            *v += value;
            if v.abs() <= f64::EPSILON {
                map.remove(&id);
            }
        }
        let mut out: Self = map.into_iter().collect();
        out.linear = match (self.linear, rhs.linear) {
            (Some(l), Some(r)) => {
                let out = l + r;
                if out.is_zero() {
                    None
                } else {
                    Some(out)
                }
            }
            (Some(l), None) | (None, Some(l)) => Some(l),
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
impl_sub_by_neg_add!(Quadratic, Linear);
impl_sub_by_neg_add!(Quadratic, f64);
impl_sub_by_neg_add!(Quadratic, Quadratic);

impl Mul for Quadratic {
    type Output = Polynomial;

    fn mul(self, rhs: Self) -> Self::Output {
        let mut terms = BTreeMap::new();
        for (id_l, value_l) in self.into_iter() {
            for (mut id_r, value_r) in rhs.clone().into_iter() {
                id_r.append(&mut id_l.clone());
                id_r.sort_unstable();
                *terms.entry(id_r).or_default() += value_l * value_r;
            }
        }
        terms.into_iter().collect()
    }
}

impl_mul_from!(Quadratic, Linear, Polynomial);
impl_mul_inverse!(Linear, Quadratic);

impl Mul<f64> for Quadratic {
    type Output = Self;

    fn mul(mut self, rhs: f64) -> Self {
        if rhs.is_zero() {
            return Self::zero();
        }
        for value in self.values.iter_mut() {
            *value *= rhs;
        }
        if let Some(linear) = self.linear {
            self.linear = Some(linear * rhs);
        } // 0 * rhs = 0
        self
    }
}

impl_mul_inverse!(f64, Quadratic);
impl_neg_by_mul!(Quadratic);

impl Arbitrary for Quadratic {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        let num_terms = 0..10_usize;
        let terms = num_terms.prop_flat_map(|num_terms| {
            proptest::collection::vec(
                (
                    (0..(2 * num_terms as u64), 0..(2 * num_terms as u64)),
                    prop_oneof![Just(0.0), -1.0..1.0],
                ),
                num_terms,
            )
        });
        let linear = Linear::arbitrary_with(());
        (terms, linear)
            .prop_map(|(terms, linear)| {
                let mut quad: Quadratic = terms.into_iter().collect();
                quad.linear = Some(linear);
                quad
            })
            .boxed()
    }
}

/// Compare coefficients in sup-norm.
impl AbsDiffEq for Quadratic {
    type Epsilon = f64;

    fn default_epsilon() -> Self::Epsilon {
        f64::default_epsilon()
    }

    fn abs_diff_eq(&self, other: &Self, epsilon: Self::Epsilon) -> bool {
        match (&self.linear, &other.linear) {
            (Some(l), Some(r)) => {
                if !l.abs_diff_eq(r, epsilon) {
                    return false;
                }
            }
            (Some(l), None) | (None, Some(l)) => {
                if !l.abs_diff_eq(&Linear::zero(), epsilon) {
                    return false;
                }
            }
            (None, None) => {}
        }
        let sub = self.clone() - other.clone();
        for (_, value) in sub.into_iter() {
            if !value.abs_diff_eq(&0.0, epsilon) {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    test_algebraic!(super::Quadratic);
}
