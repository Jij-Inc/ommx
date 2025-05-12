use super::*;
use crate::{arbitrary_constraints, arbitrary_decision_variables, Evaluate, PolynomialParameters};
use crate::{v1::State, Bound, KindParameters};
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
            arbitrary_binary_state(&analysis.used_binary()),
            arbitrary_integer_state(&analysis.used_integer(), 100),
            arbitrary_semi_integer_state(&analysis.used_semi_integer(), 100),
            arbitrary_continuous_state(&analysis.used_continuous(), 100.0),
            arbitrary_semi_continuous_state(&analysis.used_semi_continuous(), 100.0),
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

impl Arbitrary for Sense {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;
    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        prop_oneof![Just(Sense::Minimize), Just(Sense::Maximize)].boxed()
    }
}

#[derive(Debug, Clone)]
pub struct InstanceParameters {
    num_constraints: usize,
    constraint_max_id: ConstraintID,
    objective: PolynomialParameters,
    constraint: PolynomialParameters,
    kinds: KindParameters,
}

impl Default for InstanceParameters {
    fn default() -> Self {
        Self {
            num_constraints: 5,
            constraint_max_id: ConstraintID::from(10),
            objective: PolynomialParameters::default(),
            constraint: PolynomialParameters::default(),
            kinds: KindParameters::default(),
        }
    }
}

impl Arbitrary for Instance {
    type Parameters = InstanceParameters;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(p: Self::Parameters) -> Self::Strategy {
        let objective = Function::arbitrary_with(p.objective);
        let constraints =
            arbitrary_constraints(p.num_constraints, p.constraint_max_id, p.constraint);
        (objective, constraints)
            .prop_flat_map(move |(objective, constraints)| {
                // Collect all required IDs from the objective and constraints
                let mut unique_ids: FnvHashSet<VariableID> = objective
                    .required_ids()
                    .into_iter()
                    .map(VariableID::from)
                    .collect();
                for c in constraints.values() {
                    unique_ids.extend(c.function.required_ids().into_iter().map(VariableID::from));
                }
                (
                    Just(objective),
                    Just(constraints),
                    arbitrary_decision_variables(unique_ids, p.kinds.clone()),
                    Sense::arbitrary(),
                )
                    .prop_map(
                        |(objective, constraints, decision_variables, sense)| Instance {
                            objective,
                            constraints,
                            sense,
                            decision_variables,
                            constraint_hints: Default::default(),
                            parameters: Default::default(),
                            removed_constraints: Default::default(),
                            decision_variable_dependency: Default::default(),
                            description: None,
                        },
                    )
            })
            .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    proptest! {
        #[test]
        fn test_variable_id_is_defined(instance in Instance::arbitrary()) {
            for _ids in instance.objective.keys() {
                todo!()
            }
        }
    }
}
