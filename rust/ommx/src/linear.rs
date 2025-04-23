//! Rust-idiomatic Linear function

mod convert;

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Linear {
    terms: HashMap<u64, f64>,
    constant: f64,
}
