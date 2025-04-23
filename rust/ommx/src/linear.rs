//! Rust-idiomatic Linear function

mod convert;

use crate::{Coefficient, Offset, VariableID};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Linear {
    terms: HashMap<VariableID, Coefficient>,
    constant: Offset,
}
