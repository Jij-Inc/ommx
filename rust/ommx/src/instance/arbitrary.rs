use super::*;
use crate::{
    arbitrary_constraints, arbitrary_decision_variables,
    random::{arbitrary_samples, SamplesParameters},
    v1::State,
    Bounds, ConstraintIDParameters, Evaluate, KindParameters, PolynomialParameters, Sampled,
};
use fnv::FnvHashSet;
use proptest::prelude::*;
use std::collections::HashMap;

fn arbitrary_integer_state(bounds: &Bounds, max_abs: u64) -> BoxedStrategy<State> {
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

fn arbitrary_semi_integer_state(bounds: &Bounds, max_abs: u64) -> BoxedStrategy<State> {
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

fn arbitrary_continuous_state(bounds: &Bounds, max_abs: f64) -> BoxedStrategy<State> {
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

fn arbitrary_semi_continuous_state(bounds: &Bounds, max_abs: f64) -> BoxedStrategy<State> {
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
            arbitrary_integer_state(&analysis.used_binary(), 1),
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

    pub fn arbitrary_samples(&self, params: SamplesParameters) -> BoxedStrategy<Sampled<State>> {
        // FIXME: Generate Sampled<State> directly
        arbitrary_samples(params, self.arbitrary_state())
            .prop_map(|samples| samples.parse(&()).unwrap())
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
    pub constraint_ids: ConstraintIDParameters,
    pub objective: PolynomialParameters,
    pub constraint: PolynomialParameters,
    pub kinds: KindParameters,
    pub max_irrelevant_ids: usize,
}

impl InstanceParameters {
    pub fn default_lp() -> Self {
        Self {
            constraint_ids: ConstraintIDParameters::default(),
            objective: PolynomialParameters::default_linear(),
            constraint: PolynomialParameters::default_linear(),
            kinds: KindParameters::default(),
            max_irrelevant_ids: 5,
        }
    }
}

impl Default for InstanceParameters {
    fn default() -> Self {
        Self {
            constraint_ids: ConstraintIDParameters::default(),
            objective: PolynomialParameters::default(),
            constraint: PolynomialParameters::default(),
            kinds: KindParameters::default(),
            max_irrelevant_ids: 5,
        }
    }
}

impl Arbitrary for Instance {
    type Parameters = InstanceParameters;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(p: Self::Parameters) -> Self::Strategy {
        let objective = Function::arbitrary_with(p.objective);
        let constraints = arbitrary_constraints(p.constraint_ids, p.constraint);
        // Generate candidates for irrelevant IDs.
        // Since these IDs are generated without checking against the objective or constraints, some of these may be relevant.
        let max_id = p.objective.max_id().max(p.constraint.max_id());
        let irrelevant_candidates =
            proptest::collection::vec(0..=max_id.into_inner(), 0..=p.max_irrelevant_ids);
        (objective, constraints, irrelevant_candidates)
            .prop_flat_map(move |(objective, constraints, irrelevant_candidates)| {
                // Collect all required IDs from the objective and constraints
                let mut unique_ids: FnvHashSet<VariableID> =
                    objective.required_ids().into_iter().collect();
                for c in constraints.values() {
                    unique_ids.extend(c.function.required_ids().into_iter());
                }
                unique_ids.extend(irrelevant_candidates.into_iter().map(VariableID::from));
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
            for ids in instance.objective.keys() {
                for id in ids {
                    prop_assert!(instance.decision_variables.contains_key(&id));
                }
            }
            for c in instance.constraints.values() {
                for ids in c.function.keys() {
                    for id in ids {
                        prop_assert!(instance.decision_variables.contains_key(&id));
                    }
                }
            }
        }
    }
}
