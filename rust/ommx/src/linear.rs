//! Rust-idiomatic Linear function

mod add;
mod convert;
mod parse;

pub use parse::*;

use crate::{Coefficient, Offset, VariableID};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Linear {
    terms: HashMap<VariableID, Coefficient>,
    constant: Offset,
}
