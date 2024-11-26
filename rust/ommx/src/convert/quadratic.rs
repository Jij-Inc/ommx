use crate::v1::{Linear, Polynomial, Quadratic};
use approx::AbsDiffEq;
use num::Zero;
use proptest::prelude::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    ops::{Add, Mul},
};

use super::{arbitrary_coefficient, format::format_polynomial};

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

    pub fn quad_iter(&self) -> impl Iterator<Item = ((u64, u64), f64)> + '_ {
        assert_eq!(self.columns.len(), self.rows.len());
        assert_eq!(self.columns.len(), self.values.len());
        self.columns
            .iter()
            .zip(self.rows.iter())
            .zip(self.values.iter())
            .map(|((column, row), value)| ((*column, *row), *value))
    }

    /// Downcast to a linear function if the quadratic function is linear.
    pub fn as_linear(self) -> Option<Linear> {
        if self.values.iter().all(|v| v.abs() <= f64::EPSILON) {
            Some(self.linear.unwrap_or_default())
        } else {
            None
        }
    }

    /// Downcast to a constant if the quadratic function is constant.
    pub fn as_constant(self) -> Option<f64> {
        self.as_linear()?.as_constant()
    }

    pub fn degree(&self) -> usize {
        if !self.columns.is_empty() {
            2
        } else {
            self.linear.as_ref().map_or(0, |l| l.degree())
        }
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

impl<'a> IntoIterator for &'a Quadratic {
    type Item = (Vec<u64>, f64);
    type IntoIter = Box<dyn Iterator<Item = Self::Item> + 'a>;

    fn into_iter(self) -> Self::IntoIter {
        assert_eq!(self.columns.len(), self.rows.len());
        assert_eq!(self.columns.len(), self.values.len());
        let n = self.columns.len();
        let quad = (0..n).map(move |i| (vec![self.columns[i], self.rows[i]], self.values[i]));
        if let Some(linear) = &self.linear {
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
    type Parameters = (usize /* num_terms */, u64 /* max id */);
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with((num_terms, max_id): Self::Parameters) -> Self::Strategy {
        let terms = proptest::collection::vec(
            ((0..=max_id, 0..=max_id), arbitrary_coefficient()),
            num_terms,
        );
        let linear = Linear::arbitrary_with((num_terms, max_id));
        (terms, linear)
            .prop_map(|(terms, linear)| {
                let mut quad: Quadratic = terms.into_iter().collect();
                quad.linear = Some(linear);
                quad
            })
            .boxed()
    }

    fn arbitrary() -> Self::Strategy {
        (0..5_usize, 0..10_u64)
            .prop_flat_map(Self::arbitrary_with)
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

impl fmt::Display for Quadratic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_zero() {
            return write!(f, "0");
        }
        format_polynomial(f, self.into_iter())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    test_algebraic!(Quadratic);

    #[test]
    fn format() {
        let q = Quadratic::from_iter(vec![
            ((0, 1), 1.0),
            ((1, 2), -1.0),
            ((2, 0), -2.0),
            ((1, 3), 1.0 / 3.0),
        ]) + Linear::new(
            [(1, 1.0), (2, -1.0), (3, -2.0), (4, 1.0 / 3.0)].into_iter(),
            3.0,
        );
        assert_eq!(
            q.to_string(),
            "x0*x1 - 2*x0*x2 - x1*x2 + 0.3333333333333333*x1*x3 + x1 - x2 - 2*x3 + 0.3333333333333333*x4 + 3"
        );
        assert_eq!(
            format!("{:.2}", q),
            "x0*x1 - 2.00*x0*x2 - x1*x2 + 0.33*x1*x3 + x1 - x2 - 2.00*x3 + 0.33*x4 + 3.00"
        );
    }

    proptest! {
        #[test]
        fn test_as_linear_any(f in Quadratic::arbitrary()) {
            prop_assert!(dbg!(f.degree()) >= 2 || dbg!(f.as_linear()).is_some());
        }

        #[test]
        fn test_as_const_any(f in Quadratic::arbitrary()) {
            prop_assert!(dbg!(f.degree()) >= 1 || dbg!(f.as_constant()).is_some());
        }
    }
}
