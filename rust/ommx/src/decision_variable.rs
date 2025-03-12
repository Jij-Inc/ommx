use crate::{parse::*, v1};
use derive_more::{Deref, From};
use std::collections::HashMap;

/// ID for decision variable and parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, From, Deref)]
pub struct VariableID(u64);

#[derive(Debug, Clone, PartialEq)]
pub struct Bound {
    pub lower: f64,
    pub upper: f64,
}

impl Default for Bound {
    fn default() -> Self {
        Self {
            lower: f64::NEG_INFINITY,
            upper: f64::INFINITY,
        }
    }
}

impl From<v1::Bound> for Bound {
    fn from(bound: v1::Bound) -> Self {
        Self {
            lower: bound.lower,
            upper: bound.upper,
        }
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
            bound: self.bound.map(Bound::from).unwrap_or_default(),
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
