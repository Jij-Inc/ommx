use crate::v1::{Function, Linear};
use anyhow::Result;
use num::Zero;
use std::collections::HashMap;

/// Substitute decision variable with a function, `x_i = f(x_j, x_k, ...)`
pub trait Substitute {
    type Output;
    fn substitute(&self, replacements: &HashMap<u64, Function>) -> Result<Self::Output>;
}

impl Substitute for Function {
    type Output = Function;
    fn substitute(&self, replacements: &HashMap<u64, Function>) -> Result<Self::Output> {
        if replacements.is_empty() {
            return Ok(self.clone());
        }
        let mut out = Function::zero();
        for (ids, coefficient) in self {
            let mut v = Function::from(coefficient);
            for id in ids.iter() {
                if let Some(replacement) = replacements.get(id) {
                    v = v * replacement.clone();
                } else {
                    v = v * Linear::single_term(*id, 1.0);
                }
            }
            out = out + v;
        }
        Ok(out)
    }
}
