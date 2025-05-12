use crate::{parse::*, random::unique_integers, v1, Bound};
use derive_more::{Deref, From};
use fnv::{FnvHashMap, FnvHashSet};
use proptest::prelude::*;

/// ID for decision variable and parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, From, Deref)]
pub struct VariableID(u64);

impl VariableID {
    pub fn into_inner(&self) -> u64 {
        self.0
    }
}

impl From<VariableID> for u64 {
    fn from(id: VariableID) -> Self {
        id.0
    }
}

impl std::fmt::Display for VariableID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
pub struct KindParameters(FnvHashSet<Kind>);

impl KindParameters {
    pub fn new(kinds: &[Kind]) -> anyhow::Result<Self> {
        let inner: FnvHashSet<_> = kinds.iter().cloned().collect();
        if inner.is_empty() {
            Err(anyhow::anyhow!("KindParameters must not be empty"))
        } else {
            Ok(KindParameters(inner))
        }
    }
}

impl Default for KindParameters {
    fn default() -> Self {
        Self::new(&[Kind::Binary, Kind::Integer, Kind::Continuous]).unwrap()
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

impl Arbitrary for Kind {
    type Parameters = KindParameters;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(parameters: Self::Parameters) -> Self::Strategy {
        let kinds_vec: Vec<Kind> = parameters.0.into_iter().collect();
        debug_assert!(!kinds_vec.is_empty(), "KindParameters must not be empty");
        proptest::sample::select(kinds_vec).boxed()
    }
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
    pub parameters: FnvHashMap<String, String>,
    pub description: Option<String>,
}

impl Arbitrary for DecisionVariable {
    type Parameters = KindParameters;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(parameters: Self::Parameters) -> Self::Strategy {
        Kind::arbitrary_with(parameters)
            .prop_flat_map(|kind| {
                let bound_strategy = if kind == Kind::Binary {
                    Just(Bound::new(0.0, 1.0).unwrap()).boxed()
                } else {
                    Bound::arbitrary()
                };
                (Just(kind), bound_strategy)
            })
            .prop_map(|(kind, bound)| DecisionVariable {
                id: VariableID::from(0), // Should be replaced with a unique ID
                kind,
                bound,
                substituted_value: None,
                name: None,
                subscripts: Vec::new(),
                parameters: FnvHashMap::default(),
                description: None,
            })
            .boxed()
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

impl Parse for Vec<v1::DecisionVariable> {
    type Output = FnvHashMap<VariableID, DecisionVariable>;
    type Context = ();
    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let mut decision_variables = FnvHashMap::default();
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

pub fn arbitrary_unique_variable_ids(
    size: usize,
    max_id: VariableID,
) -> impl Strategy<Value = FnvHashSet<VariableID>> {
    unique_integers(0, max_id.into_inner(), size)
        .prop_map(|ids| ids.into_iter().map(VariableID::from).collect())
        .boxed()
}

pub fn arbitrary_decision_variables(
    unique_ids: FnvHashSet<VariableID>,
    parameters: KindParameters,
) -> impl Strategy<Value = FnvHashMap<VariableID, DecisionVariable>> {
    let variables = proptest::collection::vec(
        DecisionVariable::arbitrary_with(parameters),
        unique_ids.len(),
    );
    (Just(unique_ids), variables)
        .prop_map(|(ids, variables)| {
            ids.into_iter()
                .zip(variables)
                .map(|(id, mut variable)| {
                    variable.id = id;
                    (id, variable)
                })
                .collect()
        })
        .boxed()
}
