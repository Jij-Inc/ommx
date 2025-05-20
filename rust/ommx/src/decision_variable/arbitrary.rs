use super::*;

use crate::{random::unique_integers, Bound};
use fnv::{FnvHashMap, FnvHashSet};
use proptest::prelude::*;
use std::collections::BTreeMap;

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

impl Arbitrary for Kind {
    type Parameters = KindParameters;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(parameters: Self::Parameters) -> Self::Strategy {
        let kinds_vec: Vec<Kind> = parameters.0.into_iter().collect();
        debug_assert!(!kinds_vec.is_empty(), "KindParameters must not be empty");
        proptest::sample::select(kinds_vec).boxed()
    }
}

impl Arbitrary for DecisionVariable {
    type Parameters = KindParameters;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(parameters: Self::Parameters) -> Self::Strategy {
        (Kind::arbitrary_with(parameters), Bound::arbitrary())
            .prop_filter_map("Bound must be consistent with Kind", |(kind, bound)| {
                // FIXME: Constructive approach to generate bounds for faster testing
                let bound = kind.consistent_bound(bound, ATol::default())?;
                Some((kind, bound))
            })
            .prop_map(|(kind, bound)| DecisionVariable {
                id: VariableID::from(0), // Should be replaced with a unique ID, but cannot be generated here
                kind,
                bound,
                substituted_value: None, // To keep consistency in Instance level, keep this None here.
                name: None,
                subscripts: Vec::new(),
                parameters: FnvHashMap::default(),
                description: None,
            })
            .boxed()
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
) -> impl Strategy<Value = BTreeMap<VariableID, DecisionVariable>> {
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
