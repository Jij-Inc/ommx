use super::*;
use crate::{
    arbitrary_constraints, arbitrary_decision_variables, arbitrary_named_functions, linear,
    random::{arbitrary_samples, SamplesParameters},
    v1::State,
    Bounds, ConstraintIDParameters, Equality, Evaluate, FiniteDomain, IndicatorConstraintID, Kind,
    KindParameters, NamedFunctionIDParameters, OneHotConstraintID, PolynomialParameters, Sampled,
    Sos1ConstraintID,
};
use fnv::FnvHashSet;
use proptest::prelude::*;
use std::collections::{BTreeMap, BTreeSet, HashMap};

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

fn arbitrary_finite_state(domains: &BTreeMap<VariableID, &FiniteDomain>) -> BoxedStrategy<State> {
    let mut strategy = Just(HashMap::new()).boxed();
    for (id, domain) in domains {
        let raw_id = id.into_inner();
        strategy = (strategy, proptest::sample::select(domain.values().to_vec()))
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
        let usage = self.decision_variable_usage();

        (
            arbitrary_integer_state(&usage.used_binary(), 1),
            arbitrary_integer_state(&usage.used_integer(), 100),
            arbitrary_semi_integer_state(&usage.used_semi_integer(), 100),
            arbitrary_continuous_state(&usage.used_continuous(), 100.0),
            arbitrary_semi_continuous_state(&usage.used_semi_continuous(), 100.0),
            arbitrary_finite_state(&usage.used_finite_domain()),
        )
            .prop_map(
                |(binary, integer, semi_integer, continuous, semi_continuous, finite)| {
                    let mut state = HashMap::new();
                    state.extend(binary);
                    state.extend(integer);
                    state.extend(semi_integer);
                    state.extend(continuous);
                    state.extend(semi_continuous);
                    state.extend(finite);
                    state.into()
                },
            )
            .boxed()
    }

    pub fn arbitrary_samples(&self, params: SamplesParameters) -> BoxedStrategy<Sampled<State>> {
        arbitrary_samples(params, self.arbitrary_state())
    }
}

impl Arbitrary for Sense {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;
    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        prop_oneof![Just(Sense::Minimize), Just(Sense::Maximize)].boxed()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum InstanceSpace {
    /// Generate the V3 [`Instance`] domain, including special constraint
    /// families, lifecycle states, and decision-variable roles.
    FullV3,
    /// Generate only the regular-constraint subspace with no fixed/dependent
    /// variables or non-standard constraint families.
    RegularOnly,
}

/// Parameters for [`Instance`] generation.
///
/// `space` selects the domain being sampled. Use [`InstanceSpace::FullV3`]
/// when the property under test should hold for arbitrary V3 instances. Use
/// [`InstanceSpace::RegularOnly`] or one of the compatibility constructors
/// when the property has a stricter precondition such as v1 or MPS support.
#[derive(Debug, Clone)]
pub struct InstanceParameters {
    pub space: InstanceSpace,
    pub constraint_ids: ConstraintIDParameters,
    pub objective: PolynomialParameters,
    pub constraint: PolynomialParameters,
    pub named_function: PolynomialParameters,
    pub named_function_ids: NamedFunctionIDParameters,
    pub kinds: KindParameters,
    pub max_irrelevant_ids: usize,
}

impl InstanceParameters {
    /// Parameters for the full V3 [`Instance`] space. This is the default.
    ///
    /// The objective, regular constraints, named functions, and decision
    /// variables are sampled from strategies. The V3-specific structure —
    /// fixed and dependent variables, removed constraints, indicator,
    /// one-hot, and SOS1 families, parameters, description, and annotations
    /// — is injected deterministically, so each of those dimensions is
    /// exercised at a single representative point (smoke coverage) rather
    /// than sampled. Every generated instance contains all V3 features;
    /// feature-absent combinations are covered by the narrower spaces such
    /// as [`Self::regular_only`].
    pub fn full_v3() -> Self {
        Self {
            space: InstanceSpace::FullV3,
            constraint_ids: ConstraintIDParameters::default(),
            named_function_ids: NamedFunctionIDParameters::default(),
            objective: PolynomialParameters::default(),
            constraint: PolynomialParameters::default(),
            named_function: PolynomialParameters::default(),
            kinds: KindParameters::new(&[
                Kind::Binary,
                Kind::Integer,
                Kind::Continuous,
                Kind::SemiInteger,
                Kind::SemiContinuous,
                Kind::FiniteDomain,
            ])
            .unwrap(),
            max_irrelevant_ids: 5,
        }
    }

    /// Parameters for the regular-constraint subspace: no special constraint
    /// families, no removed constraints, and no fixed or dependent variables.
    pub fn regular_only() -> Self {
        Self {
            space: InstanceSpace::RegularOnly,
            constraint_ids: ConstraintIDParameters::default(),
            named_function_ids: NamedFunctionIDParameters::default(),
            objective: PolynomialParameters::default(),
            constraint: PolynomialParameters::default(),
            named_function: PolynomialParameters::default(),
            kinds: KindParameters::default(),
            max_irrelevant_ids: 5,
        }
    }

    /// Parameters for instances that round-trip through the v1 protobuf
    /// format losslessly.
    ///
    /// Currently identical to [`Self::regular_only`]; the claim is pinned by
    /// the `instance_roundtrip` property test, which fails if the two spaces
    /// drift apart.
    pub fn v1_compatible() -> Self {
        Self::regular_only()
    }

    /// Parameters for QCQP instances that the MPS writer supports: regular
    /// constraints only and no named functions.
    pub fn mps_compatible_qcqp() -> Self {
        Self {
            named_function_ids: NamedFunctionIDParameters::new(0, 0.into()).unwrap(),
            ..Self::default_qcqp()
        }
    }

    /// Default parameter for Linear Programming (LP), i.e. linear objective and linear constraints.
    pub fn default_lp() -> Self {
        Self {
            space: InstanceSpace::RegularOnly,
            constraint_ids: ConstraintIDParameters::default(),
            named_function_ids: NamedFunctionIDParameters::default(),
            named_function: PolynomialParameters::default_linear(),
            objective: PolynomialParameters::default_linear(),
            constraint: PolynomialParameters::default_linear(),
            kinds: KindParameters::default(),
            max_irrelevant_ids: 5,
        }
    }

    /// Default parameter for Quadratic Programming (QP), i.e. quadratic objective and linear constraints.
    pub fn default_qp() -> Self {
        Self {
            space: InstanceSpace::RegularOnly,
            constraint_ids: ConstraintIDParameters::default(),
            named_function_ids: NamedFunctionIDParameters::default(),
            objective: PolynomialParameters::default_quadratic(),
            constraint: PolynomialParameters::default_linear(),
            named_function: PolynomialParameters::default_linear(),
            kinds: KindParameters::default(),
            max_irrelevant_ids: 5,
        }
    }

    /// Default parameter for Quadratically Constrained Quadratic Programming (QCQP), i.e. quadratic objective and quadratic constraints.
    pub fn default_qcqp() -> Self {
        Self {
            space: InstanceSpace::RegularOnly,
            constraint_ids: ConstraintIDParameters::default(),
            named_function_ids: NamedFunctionIDParameters::default(),
            objective: PolynomialParameters::default_quadratic(),
            constraint: PolynomialParameters::default_quadratic(),
            named_function: PolynomialParameters::default_quadratic(),
            kinds: KindParameters::default(),
            max_irrelevant_ids: 5,
        }
    }
}

impl Default for InstanceParameters {
    fn default() -> Self {
        Self::full_v3()
    }
}

fn fresh_variable_ids(existing: &mut FnvHashSet<VariableID>, size: usize) -> Vec<VariableID> {
    let mut ids = Vec::with_capacity(size);
    let mut raw_id = 0_u64;
    while ids.len() < size {
        let id = VariableID::from(raw_id);
        if existing.insert(id) {
            ids.push(id);
        }
        raw_id = raw_id
            .checked_add(1)
            .expect("exhausted variable ID space while generating fresh IDs");
    }
    ids
}

fn next_constraint_id(constraints: &BTreeMap<ConstraintID, Constraint>) -> ConstraintID {
    constraints
        .keys()
        .next_back()
        .map(|id| ConstraintID::from(id.into_inner() + 1))
        .unwrap_or_else(|| ConstraintID::from(0))
}

fn arbitrary_removed_reason() -> crate::constraint::RemovedReason {
    crate::constraint::RemovedReason {
        reason: "arbitrary".to_string(),
        parameters: Default::default(),
    }
}

impl Arbitrary for Instance {
    type Parameters = InstanceParameters;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(p: Self::Parameters) -> Self::Strategy {
        let objective = Function::arbitrary_with(p.objective);
        let constraints = arbitrary_constraints(p.constraint_ids, p.constraint);
        let named_functions = arbitrary_named_functions(p.named_function_ids, p.named_function);
        let space = p.space;
        let kinds = p.kinds.clone();
        // Generate candidates for irrelevant IDs.
        // Since these IDs are generated without checking against the objective or constraints, some of these may be relevant.
        let max_id = p
            .objective
            .max_id()
            .max(p.constraint.max_id())
            .max(p.named_function.max_id());
        let irrelevant_candidates =
            proptest::collection::vec(0..=max_id.into_inner(), 0..=p.max_irrelevant_ids);
        (
            objective,
            constraints,
            named_functions,
            irrelevant_candidates,
        )
            .prop_flat_map(
                move |(objective, constraints, named_functions, irrelevant_candidates)| {
                    // Collect all required IDs from the objective and constraints
                    let mut unique_ids: FnvHashSet<VariableID> =
                        objective.required_ids().into_iter().collect();
                    for c in constraints.values() {
                        unique_ids.extend(c.function().required_ids());
                    }
                    for nf in named_functions.values() {
                        unique_ids.extend(nf.function.required_ids());
                    }
                    unique_ids.extend(irrelevant_candidates.into_iter().map(VariableID::from));
                    // Reserving the fresh IDs in `unique_ids` means
                    // `arbitrary_decision_variables` also generates variables
                    // for them; those are overwritten below with the kinds the
                    // V3 structure requires (binary/continuous).
                    let full_v3_ids = (space == InstanceSpace::FullV3)
                        .then(|| fresh_variable_ids(&mut unique_ids, 12));
                    (
                        Just(objective),
                        Just(constraints),
                        Just(named_functions),
                        Just(full_v3_ids),
                        arbitrary_decision_variables(unique_ids, kinds.clone()),
                        Sense::arbitrary(),
                    )
                        .prop_map(
                            move |(
                                objective,
                                constraints,
                                named_functions,
                                full_v3_ids,
                                decision_variables,
                                sense,
                            )| {
                                let mut decision_variables = decision_variables;
                                let mut variable_labels = VariableLabelStore::default();
                                let mut named_function_labels =
                                    crate::named_function::NamedFunctionLabelStore::default();
                                let mut constraint_context =
                                    ConstraintContextStore::<ConstraintID>::default();
                                let mut removed_constraints = BTreeMap::new();
                                let mut fixed_decision_variable_values = BTreeMap::new();
                                let mut decision_variable_dependency =
                                    AcyclicAssignments::default();
                                let mut indicator_constraints = BTreeMap::new();
                                let mut removed_indicator_constraints = BTreeMap::new();
                                let mut indicator_constraint_context =
                                    ConstraintContextStore::<IndicatorConstraintID>::default();
                                let mut one_hot_constraints = BTreeMap::new();
                                let mut one_hot_constraint_context =
                                    ConstraintContextStore::<OneHotConstraintID>::default();
                                let mut sos1_constraints = BTreeMap::new();
                                let mut sos1_constraint_context =
                                    ConstraintContextStore::<Sos1ConstraintID>::default();

                                if let Some((id, _)) = decision_variables.iter().next() {
                                    variable_labels.set_name(*id, "arbitrary_variable");
                                }
                                if let Some(id) = named_functions.keys().next().copied() {
                                    named_function_labels.set_name(id, "arbitrary_named_function");
                                }
                                if let Some(id) = constraints.keys().next().copied() {
                                    constraint_context.set_name(id, "arbitrary_constraint");
                                }

                                if let Some(ids) = full_v3_ids {
                                    let fixed_id = ids[0];
                                    let dependent_id = ids[1];
                                    let indicator_var = ids[2];
                                    let indicator_removed_var = ids[3];
                                    let indicator_body_var = ids[4];
                                    let removed_indicator_body_var = ids[5];
                                    let one_hot_active_a = ids[6];
                                    let one_hot_active_b = ids[7];
                                    let one_hot_removed_a = ids[8];
                                    let one_hot_removed_b = ids[9];
                                    let sos1_active_var = ids[10];
                                    let sos1_removed_var = ids[11];

                                    decision_variables.insert(fixed_id, DecisionVariable::binary());
                                    fixed_decision_variable_values.insert(fixed_id, 0.0);
                                    variable_labels.set_name(fixed_id, "arbitrary_fixed");

                                    decision_variables
                                        .insert(dependent_id, DecisionVariable::continuous());
                                    decision_variable_dependency =
                                        AcyclicAssignments::new([(dependent_id, Function::Zero)])
                                            .expect("constant assignment is acyclic");
                                    variable_labels.set_name(dependent_id, "arbitrary_dependent");

                                    for id in [
                                        indicator_var,
                                        indicator_removed_var,
                                        indicator_body_var,
                                        removed_indicator_body_var,
                                        one_hot_active_a,
                                        one_hot_active_b,
                                        one_hot_removed_a,
                                        one_hot_removed_b,
                                        sos1_active_var,
                                        sos1_removed_var,
                                    ] {
                                        decision_variables.insert(id, DecisionVariable::binary());
                                    }

                                    let removed_id = next_constraint_id(&constraints);
                                    removed_constraints.insert(
                                        removed_id,
                                        (
                                            Constraint::equal_to_zero(Function::Zero),
                                            arbitrary_removed_reason(),
                                        ),
                                    );
                                    constraint_context
                                        .set_name(removed_id, "arbitrary_removed_constraint");

                                    let indicator_id = IndicatorConstraintID::from(0);
                                    indicator_constraints.insert(
                                        indicator_id,
                                        IndicatorConstraint::new(
                                            indicator_var,
                                            Equality::LessThanOrEqualToZero,
                                            Function::from(
                                                linear!(indicator_body_var.into_inner()),
                                            ),
                                        ),
                                    );
                                    indicator_constraint_context
                                        .set_name(indicator_id, "arbitrary_indicator");

                                    let removed_indicator_id = IndicatorConstraintID::from(1);
                                    removed_indicator_constraints.insert(
                                        removed_indicator_id,
                                        (
                                            IndicatorConstraint::new(
                                                indicator_removed_var,
                                                Equality::EqualToZero,
                                                Function::from(linear!(
                                                    removed_indicator_body_var.into_inner()
                                                )),
                                            ),
                                            arbitrary_removed_reason(),
                                        ),
                                    );
                                    indicator_constraint_context.set_name(
                                        removed_indicator_id,
                                        "arbitrary_removed_indicator",
                                    );

                                    let one_hot_id = OneHotConstraintID::from(0);
                                    one_hot_constraints.insert(
                                        one_hot_id,
                                        OneHotConstraint::new(BTreeSet::from([
                                            one_hot_active_a,
                                            one_hot_active_b,
                                        ]))
                                        .expect("one-hot set is non-empty"),
                                    );
                                    one_hot_constraint_context
                                        .set_name(one_hot_id, "arbitrary_one_hot");

                                    let removed_one_hot_id = OneHotConstraintID::from(1);
                                    one_hot_constraints.insert(
                                        removed_one_hot_id,
                                        OneHotConstraint::new(BTreeSet::from([
                                            one_hot_removed_a,
                                            one_hot_removed_b,
                                        ]))
                                        .expect("one-hot set is non-empty"),
                                    );
                                    one_hot_constraint_context
                                        .set_name(removed_one_hot_id, "arbitrary_removed_one_hot");

                                    // The SOS1 sets deliberately share one
                                    // variable with the one-hot sets so the
                                    // generated instances cover variables that
                                    // belong to more than one special
                                    // constraint family.
                                    let sos1_id = Sos1ConstraintID::from(0);
                                    sos1_constraints.insert(
                                        sos1_id,
                                        Sos1Constraint::new(BTreeSet::from([
                                            sos1_active_var,
                                            one_hot_active_b,
                                        ]))
                                        .expect("SOS1 set is non-empty"),
                                    );
                                    sos1_constraint_context.set_name(sos1_id, "arbitrary_sos1");

                                    let removed_sos1_id = Sos1ConstraintID::from(1);
                                    sos1_constraints.insert(
                                        removed_sos1_id,
                                        Sos1Constraint::new(BTreeSet::from([
                                            sos1_removed_var,
                                            one_hot_removed_b,
                                        ]))
                                        .expect("SOS1 set is non-empty"),
                                    );
                                    sos1_constraint_context
                                        .set_name(removed_sos1_id, "arbitrary_removed_sos1");
                                }

                                let mut instance = Instance::builder()
                                    .sense(sense)
                                    .objective(objective)
                                    .decision_variables(decision_variables)
                                    .variable_labels(variable_labels)
                                    .fixed_decision_variable_values(fixed_decision_variable_values)
                                    .constraints(constraints)
                                    .constraint_context(constraint_context)
                                    .removed_constraints(removed_constraints)
                                    .indicator_constraints(indicator_constraints)
                                    .indicator_constraint_context(indicator_constraint_context)
                                    .removed_indicator_constraints(removed_indicator_constraints)
                                    .one_hot_constraints(one_hot_constraints)
                                    .one_hot_constraint_context(one_hot_constraint_context)
                                    .sos1_constraints(sos1_constraints)
                                    .sos1_constraint_context(sos1_constraint_context)
                                    .named_functions(named_functions)
                                    .named_function_labels(named_function_labels)
                                    .decision_variable_dependency(decision_variable_dependency)
                                    .build()
                                    .expect("arbitrary Instance must satisfy builder invariants");

                                if space == InstanceSpace::FullV3 {
                                    instance
                                        .convert_one_hot_to_constraint(OneHotConstraintID::from(1))
                                        .expect("arbitrary removed one-hot conversion must work");
                                    instance
                                        .convert_sos1_to_constraints(Sos1ConstraintID::from(1))
                                        .expect("arbitrary removed SOS1 conversion must work");
                                    let mut parameters = v1::Parameters {
                                        entries: HashMap::new(),
                                    };
                                    parameters.entries.insert(0, 1.0);
                                    instance.parameters = Some(parameters);
                                    instance.description = Some(v1::instance::Description {
                                        name: Some("arbitrary instance".to_string()),
                                        description: Some(
                                            "generated by Instance::arbitrary".to_string(),
                                        ),
                                        authors: vec!["ommx".to_string()],
                                        created_by: Some("ommx".to_string()),
                                        ..Default::default()
                                    });
                                    instance.annotations.insert(
                                        "org.ommx.user.arbitrary".to_string(),
                                        "true".to_string(),
                                    );
                                }

                                instance
                            },
                        )
                },
            )
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
            for c in instance.constraints().values() {
                for ids in c.function().keys() {
                    for id in ids {
                        prop_assert!(instance.decision_variables.contains_key(&id));
                    }
                }
            }
            for (c, _) in instance.removed_constraints().values() {
                for id in c.required_ids() {
                    prop_assert!(instance.decision_variables.contains_key(&id));
                }
            }
            for c in instance.indicator_constraints().values() {
                for id in c.required_ids() {
                    prop_assert!(instance.decision_variables.contains_key(&id));
                }
            }
            for (c, _) in instance.removed_indicator_constraints().values() {
                for id in c.required_ids() {
                    prop_assert!(instance.decision_variables.contains_key(&id));
                }
            }
            for c in instance.one_hot_constraints().values() {
                for id in c.required_ids() {
                    prop_assert!(instance.decision_variables.contains_key(&id));
                }
            }
            for (c, _) in instance.removed_one_hot_constraints().values() {
                for id in c.required_ids() {
                    prop_assert!(instance.decision_variables.contains_key(&id));
                }
            }
            for c in instance.sos1_constraints().values() {
                for id in c.required_ids() {
                    prop_assert!(instance.decision_variables.contains_key(&id));
                }
            }
            for (c, _) in instance.removed_sos1_constraints().values() {
                for id in c.required_ids() {
                    prop_assert!(instance.decision_variables.contains_key(&id));
                }
            }
            for nf in instance.named_functions().values() {
                for id in nf.required_ids() {
                    prop_assert!(instance.decision_variables.contains_key(&id));
                }
            }
            for id in instance.decision_variable_dependency().keys() {
                prop_assert!(instance.decision_variables.contains_key(&id));
            }
            for id in instance.decision_variable_dependency().required_ids() {
                prop_assert!(instance.decision_variables.contains_key(&id));
            }
        }

        // Generator-regression test: the asserted structure is injected
        // deterministically by the FullV3 space, so this pins the generator's
        // coverage of V3 features rather than a domain property.
        #[test]
        fn full_v3_space_exercises_v3_instance_state(instance in Instance::arbitrary()) {
            prop_assert!(!instance.fixed_decision_variable_values().is_empty());
            prop_assert!(!instance.decision_variable_dependency().is_empty());
            prop_assert!(!instance.removed_constraints().is_empty());
            prop_assert!(!instance.indicator_constraints().is_empty());
            prop_assert!(!instance.removed_indicator_constraints().is_empty());
            prop_assert!(!instance.one_hot_constraints().is_empty());
            prop_assert!(!instance.removed_one_hot_constraints().is_empty());
            prop_assert!(!instance.sos1_constraints().is_empty());
            prop_assert!(!instance.removed_sos1_constraints().is_empty());
            prop_assert!(instance.parameters.is_some());
            prop_assert!(instance.description.is_some());
            prop_assert!(!instance.annotations.is_empty());
            prop_assert!(
                instance
                    .indicator_constraints()
                    .values()
                    .any(|constraint| !constraint.function().required_ids().is_empty())
            );
            prop_assert!(
                instance
                    .removed_indicator_constraints()
                    .values()
                    .any(|(constraint, _)| !constraint.function().required_ids().is_empty())
            );
        }
    }
}
