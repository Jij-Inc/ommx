mod one_hot;
mod sos1;

pub use one_hot::OneHot;
pub use sos1::Sos1;

use crate::{
    parse::{Parse, ParseError},
    v1, Constraint, ConstraintID, DecisionVariable, RemovedConstraint, VariableID,
};
use std::collections::BTreeMap;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ConstraintHints {
    pub one_hot_constraints: Vec<OneHot>,
    pub sos1_constraints: Vec<Sos1>,
}

impl ConstraintHints {
    pub fn is_empty(&self) -> bool {
        self.one_hot_constraints.is_empty() && self.sos1_constraints.is_empty()
    }
}

impl Parse for v1::ConstraintHints {
    type Output = ConstraintHints;
    type Context = (
        BTreeMap<VariableID, DecisionVariable>,
        BTreeMap<ConstraintID, Constraint>,
        BTreeMap<ConstraintID, RemovedConstraint>,
    );
    fn parse(self, context: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.ConstraintHints";
        let one_hot_constraints = self
            .one_hot_constraints
            .into_iter()
            .map(|c| c.parse_as(context, message, "one_hot_constraints"))
            .collect::<Result<Vec<_>, ParseError>>()?;
        let sos1_constraints = self
            .sos1_constraints
            .into_iter()
            .map(|c| c.parse_as(context, message, "sos1_constraints"))
            .collect::<Result<_, ParseError>>()?;
        Ok(ConstraintHints {
            one_hot_constraints,
            sos1_constraints,
        })
    }
}

impl From<ConstraintHints> for v1::ConstraintHints {
    fn from(value: ConstraintHints) -> Self {
        Self {
            one_hot_constraints: value
                .one_hot_constraints
                .into_iter()
                .map(|oh| oh.into())
                .collect(),
            sos1_constraints: value
                .sos1_constraints
                .into_iter()
                .map(|s| s.into())
                .collect(),
        }
    }
}
