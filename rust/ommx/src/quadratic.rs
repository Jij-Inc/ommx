mod convert;
mod pair;

pub use pair::VariableIDPair;

use crate::{Coefficient, Linear};
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

impl Quadratic {
    /// The maximum absolute value of the coefficients including the constant.
    ///
    /// `None` means this quadratic function is exactly zero.
    pub fn max_coefficient_abs(&self) -> Option<Coefficient> {
        self.quad
            .values()
            .map(|coefficient| coefficient.abs())
            .chain(self.linear.max_coefficient_abs())
            .max()
    }
}
