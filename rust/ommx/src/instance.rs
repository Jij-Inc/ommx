use crate::{Constraint, Function};

#[derive(Debug, Clone, PartialEq)]
pub struct Instance {
    objective: Function,
    constraints: Vec<Constraint>,
}
