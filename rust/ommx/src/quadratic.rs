mod add;
mod convert;
mod pair;

pub use pair::VariableIDPair;

use crate::{Coefficient, Linear, PolynomialProperties};
use std::collections::HashMap;

/// Quadratic function
///
/// - Since the decision variable may be non-binary, we keep the squared terms as it is.
/// - This represents up-to quadratic function, i.e. quadratic term can be empty.
///
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Quadratic {
    quad: HashMap<VariableIDPair, Coefficient>,
    linear: Linear,
}

impl PolynomialProperties for Quadratic {
    fn degree(&self) -> u32 {
        if !self.quad.is_empty() {
            2
        } else {
            self.linear.degree()
        }
    }

    fn num_terms(&self) -> usize {
        self.quad.len() + self.linear.num_terms()
    }

    fn max_coefficient_abs(&self) -> Option<Coefficient> {
        self.quad
            .values()
            .map(|coefficient| coefficient.abs())
            .chain(self.linear.max_coefficient_abs())
            .max()
    }
}
