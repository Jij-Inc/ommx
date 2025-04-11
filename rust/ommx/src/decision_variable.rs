use crate::{parse::*, v1, Bound};
use derive_more::{Deref, From};
use std::collections::HashMap;

/// ID for decision variable and parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, From, Deref)]
pub struct VariableID(u64);

impl std::fmt::Display for VariableID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    Continuous,
    Integer,
    Binary,
    SemiContinuous,
    SemiInteger,
}

impl Parse for v1::decision_variable::Kind {
    type Output = Kind;
    type Context = ();
    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        use v1::decision_variable::Kind::*;
        match self {
            Unspecified => Err(RawParseError::UnspecifiedEnum {
                enum_name: "ommx.v1.decision_variable.Kind",
            }
            .into()),
            Continuous => Ok(Kind::Continuous),
            Integer => Ok(Kind::Integer),
            Binary => Ok(Kind::Binary),
            SemiContinuous => Ok(Kind::SemiContinuous),
            SemiInteger => Ok(Kind::SemiInteger),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DecisionVariable {
    pub id: VariableID,
    pub kind: Kind,
    pub bound: Bound,

    pub substituted_value: Option<f64>,

    pub name: Option<String>,
    pub subscripts: Vec<i64>,
    pub parameters: HashMap<String, String>,
    pub description: Option<String>,
}

impl Parse for v1::DecisionVariable {
    type Output = DecisionVariable;
    type Context = ();
    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.DecisionVariable";
        Ok(DecisionVariable {
            id: VariableID(self.id),
            kind: self.kind().parse_as(&(), message, "kind")?,
            bound: self
                .bound
                .unwrap_or_default()
                .parse_as(&(), message, "bound")?,
            substituted_value: self.substituted_value,
            name: self.name,
            subscripts: self.subscripts,
            parameters: self.parameters,
            description: self.description,
        })
    }
}

impl Parse for Vec<v1::DecisionVariable> {
    type Output = HashMap<VariableID, DecisionVariable>;
    type Context = ();
    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let mut decision_variables = HashMap::new();
        for v in self {
            let v: DecisionVariable = v.parse(&())?;
            let id = v.id;
            if decision_variables.insert(id, v).is_some() {
                return Err(RawParseError::DuplicatedVariableID { id }.into());
            }
        }
        Ok(decision_variables)
    }
}
