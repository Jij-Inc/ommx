use crate::{Coefficient, VariableID};
use std::{collections::HashMap, hash::Hash};

pub trait Monomial: Hash {}

pub struct Polynomial<M: Monomial> {
    terms: HashMap<M, Coefficient>,
}

/// Linear function only contains monomial of degree 1 or constant
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LinearMonomial {
    Variable(VariableID),
    Constant,
}

impl Monomial for LinearMonomial {}
