use super::*;

use crate::{parse::*, v1};
use std::collections::BTreeMap;

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

impl TryFrom<v1::decision_variable::Kind> for Kind {
    type Error = ParseError;
    fn try_from(value: v1::decision_variable::Kind) -> Result<Self, Self::Error> {
        value.parse(&())
    }
}

impl From<Kind> for v1::decision_variable::Kind {
    fn from(kind: Kind) -> Self {
        use v1::decision_variable::Kind::*;
        match kind {
            Kind::Continuous => Continuous,
            Kind::Integer => Integer,
            Kind::Binary => Binary,
            Kind::SemiContinuous => SemiContinuous,
            Kind::SemiInteger => SemiInteger,
        }
    }
}

impl From<Kind> for i32 {
    fn from(kind: Kind) -> Self {
        v1::decision_variable::Kind::from(kind) as i32
    }
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
            parameters: self.parameters.into_iter().collect(),
            description: self.description,
        })
    }
}

impl TryFrom<v1::DecisionVariable> for DecisionVariable {
    type Error = ParseError;
    fn try_from(value: v1::DecisionVariable) -> Result<Self, Self::Error> {
        value.parse(&())
    }
}

impl Parse for Vec<v1::DecisionVariable> {
    type Output = BTreeMap<VariableID, DecisionVariable>;
    type Context = ();
    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let mut decision_variables = BTreeMap::default();
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

impl From<DecisionVariable> for v1::DecisionVariable {
    fn from(
        DecisionVariable {
            id,
            kind,
            bound,
            substituted_value,
            name,
            subscripts,
            parameters,
            description,
        }: DecisionVariable,
    ) -> Self {
        Self {
            id: id.into_inner(),
            kind: kind.into(),
            bound: Some(bound.into()),
            substituted_value,
            name,
            subscripts,
            parameters: parameters.into_iter().collect(),
            description,
        }
    }
}
