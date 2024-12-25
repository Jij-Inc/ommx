use crate::v1::Function;
use anyhow::Result;
use std::collections::HashMap;

/// Substitute decision variable with a function, `x_i = f(x_j, x_k, ...)`
pub trait Substitute {
    type Output;
    fn substitute(&self, replacements: &HashMap<u64, Function>) -> Result<Self::Output>;
}
