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
        let kind = self.kind().parse_as(&(), message, "kind")?;
        let bound = self
            .bound
            .unwrap_or_default()
            .parse_as(&(), message, "bound")?;
        let mut dv = DecisionVariable::new(
            VariableID(self.id),
            kind,
            bound,
            self.substituted_value,
            1e-6, // FIXME: user should provide this
        )
        .map_err(|e| RawParseError::InvalidDecisionVariable(e).context(message, "bound"))?;
        dv.name = self.name;
        dv.subscripts = self.subscripts;
        dv.parameters = self.parameters.into_iter().collect();
        dv.description = self.description;
        Ok(dv)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_decision_variable() {
        let dv = v1::DecisionVariable {
            id: 1,
            kind: v1::decision_variable::Kind::Integer as i32,
            bound: Some(v1::Bound {
                lower: 1.1,
                upper: 1.9,
            }),
            ..Default::default()
        };
        insta::assert_snapshot!(dv.parse(&()).unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.DecisionVariable[bound]
        Bound for ID=1 is inconsistent to kind: kind=Integer, bound=[1.1, 1.9]
        "###);
    }
}
