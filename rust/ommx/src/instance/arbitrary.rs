use super::*;
use crate::{v1::State, Bound};
use fnv::FnvHashSet;
use proptest::prelude::*;
use std::collections::HashMap;

fn arbitrary_binary_state(ids: &FnvHashSet<VariableID>) -> BoxedStrategy<State> {
    let mut strategy = Just(HashMap::new()).boxed();
    for id in ids {
        let raw_id = id.into_inner();
        strategy = (strategy, any::<bool>())
            .prop_map(move |(mut state, value)| {
                state.insert(raw_id, if value { 1.0 } else { 0.0 });
                state
            })
            .boxed();
    }
    strategy.prop_map(|state| state.into()).boxed()
}

fn arbitrary_integer_state(
    bounds: &FnvHashMap<VariableID, Bound>,
    max_abs: u64,
) -> BoxedStrategy<State> {
    let mut strategy = Just(HashMap::new()).boxed();
    for (id, bound) in bounds {
        let raw_id = id.into_inner();
        strategy = (strategy, bound.arbitrary_containing_integer(max_abs))
            .prop_map(move |(mut state, value)| {
                state.insert(raw_id, value as f64);
                state
            })
            .boxed();
    }
    strategy.prop_map(|state| state.into()).boxed()
}

fn arbitrary_semi_integer_state(
    bounds: &FnvHashMap<VariableID, Bound>,
    max_abs: u64,
) -> BoxedStrategy<State> {
    let mut strategy = Just(HashMap::new()).boxed();
    for (id, bound) in bounds {
        let raw_id = id.into_inner();
        strategy = (
            strategy,
            prop_oneof![bound.arbitrary_containing_integer(max_abs), Just(0)],
        )
            .prop_map(move |(mut state, value)| {
                state.insert(raw_id, value as f64);
                state
            })
            .boxed();
    }
    strategy.prop_map(|state| state.into()).boxed()
}

fn arbitrary_continuous_state(
    bounds: &FnvHashMap<VariableID, Bound>,
    max_abs: f64,
) -> BoxedStrategy<State> {
    let mut strategy = Just(HashMap::new()).boxed();
    for (id, bound) in bounds {
        let raw_id = id.into_inner();
        strategy = (strategy, bound.arbitrary_containing(max_abs))
            .prop_map(move |(mut state, value)| {
                state.insert(raw_id, value);
                state
            })
            .boxed();
    }
    strategy.prop_map(|state| state.into()).boxed()
}

fn arbitrary_semi_continuous_state(
    bounds: &FnvHashMap<VariableID, Bound>,
    max_abs: f64,
) -> BoxedStrategy<State> {
    let mut strategy = Just(HashMap::new()).boxed();
    for (id, bound) in bounds {
        let raw_id = id.into_inner();
        strategy = (
            strategy,
            prop_oneof![bound.arbitrary_containing(max_abs), Just(0.0)],
        )
            .prop_map(move |(mut state, value)| {
                state.insert(raw_id, value);
                state
            })
            .boxed();
    }
    strategy.prop_map(|state| state.into()).boxed()
}

impl Instance {
    pub fn arbitrary_state(&self) -> BoxedStrategy<State> {
        let analysis = self.analyze_decision_variables();
        (
            arbitrary_binary_state(analysis.binary()),
            arbitrary_integer_state(analysis.integer(), 100),
            arbitrary_semi_integer_state(analysis.semi_integer(), 100),
            arbitrary_continuous_state(analysis.continuous(), 100.0),
            arbitrary_semi_continuous_state(analysis.semi_continuous(), 100.0),
        )
            .prop_map(
                |(binary, integer, semi_integer, continuous, semi_continuous)| {
                    let mut state = HashMap::new();
                    state.extend(binary);
                    state.extend(integer);
                    state.extend(semi_integer);
                    state.extend(continuous);
                    state.extend(semi_continuous);
                    state.into()
                },
            )
            .boxed()
    }
}
