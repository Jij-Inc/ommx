//! Rust-idiomatic Linear function

mod add;
mod approx;
mod arbitrary;
mod convert;
mod mul;
mod parse;

use crate::{Coefficient, Offset, VariableID};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Linear {
    terms: HashMap<VariableID, Coefficient>,
    constant: Offset,
}

impl Linear {
    /// The maximum absolute value of the coefficients including the constant.
    ///
    /// `None` means this linear function is exactly zero.
    pub fn max_coefficient_abs(&self) -> Option<Coefficient> {
        self.terms
            .values()
            .map(|coefficient| coefficient.abs())
            .chain(self.constant.try_into().map(|c: Coefficient| c.abs()))
            .max()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use maplit::*;
    use std::collections::HashMap;

    #[test]
    fn test_max_coefficient_abs() {
        assert_eq!(Linear::default().max_coefficient_abs(), None);

        let linear = Linear {
            terms: HashMap::new(),
            constant: (-1.0).try_into().unwrap(),
        };
        assert_eq!(linear.max_coefficient_abs(), Some(1.0.try_into().unwrap()));

        let linear = Linear {
            terms: hashmap! {
                1.into() => 0.5.try_into().unwrap(),
                2.into() => (-1.5).try_into().unwrap(),
            },
            constant: (-1.0).try_into().unwrap(),
        };
        assert_eq!(linear.max_coefficient_abs(), Some(1.5.try_into().unwrap()));

        let linear = Linear {
            terms: hashmap! {
                1.into() => 0.5.try_into().unwrap(),
                2.into() => (-1.5).try_into().unwrap(),
            },
            constant: (-2.0).try_into().unwrap(),
        };
        assert_eq!(linear.max_coefficient_abs(), Some(2.0.try_into().unwrap()));
    }
}
