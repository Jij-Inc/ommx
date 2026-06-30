use super::*;
use crate::Result;
use crate::{
    constraint::RemovedReason, constraint_type::ActiveConstraintUpdate, ATol, Bound, Evaluate,
    Kind, Propagate, PropagateOutcome, VariableIDSet,
};
use std::collections::BTreeMap;

fn ensure_state_value_is_finite(var_id: u64, value: f64) -> Result<()> {
    if !value.is_finite() {
        crate::bail!(
            { var_id, value },
            "state value for variable ID={var_id} must be finite (value={value})",
        );
    }
    Ok(())
}

fn ensure_instance_value_is_finite(var_id: VariableID, value: f64) -> Result<()> {
    if !value.is_finite() {
        crate::bail!(
            { var_id = ?var_id, value },
            "instance value for variable {var_id:?} must be finite (value={value})",
        );
    }
    Ok(())
}

fn values_are_consistent(left: f64, right: f64, atol: ATol) -> bool {
    left.is_finite() && right.is_finite() && (left - right).abs() <= *atol
}

fn evaluate_decision_variable(
    id: VariableID,
    decision_variable: &DecisionVariable,
    state: &v1::State,
) -> Result<crate::EvaluatedDecisionVariable> {
    let value = state
        .entries
        .get(&id.into_inner())
        .copied()
        .ok_or_else(|| crate::error!("Variable ID {id} not found in state"))?;
    Ok(crate::EvaluatedDecisionVariable::new(
        id,
        decision_variable.clone(),
        value,
    )?)
}

fn evaluate_decision_variable_samples(
    id: VariableID,
    decision_variable: &DecisionVariable,
    samples: &crate::Sampled<v1::State>,
) -> Result<crate::SampledDecisionVariable> {
    let variable_id = id.into_inner();
    let mut grouped_values: BTreeMap<ordered_float::OrderedFloat<f64>, Vec<crate::SampleID>> =
        BTreeMap::new();
    for (sample_id, state) in samples.iter() {
        if let Some(value) = state.entries.get(&variable_id) {
            grouped_values
                .entry(ordered_float::OrderedFloat(*value))
                .or_default()
                .push(*sample_id);
        }
    }

    let (ids, values): (Vec<Vec<crate::SampleID>>, Vec<f64>) = grouped_values
        .into_iter()
        .map(|(value, ids)| (ids, value.into_inner()))
        .unzip();
    let samples = crate::Sampled::new(ids, values)?;
    Ok(crate::SampledDecisionVariable::new(
        id,
        decision_variable.clone(),
        samples,
    )?)
}

/// Merge additional variable fixings from propagation into `expanded` state.
///
/// Returns `Err` if any fixing conflicts with an existing value in `expanded`
/// (outside of `atol`), which indicates infeasibility discovered during
/// propagation.
fn merge_state(
    expanded: &mut v1::State,
    additional: v1::State,
    atol: ATol,
    changed: &mut bool,
) -> Result<()> {
    for (var_id, value) in additional.entries {
        ensure_state_value_is_finite(var_id, value)?;
        if let Some(&existing) = expanded.entries.get(&var_id) {
            ensure_state_value_is_finite(var_id, existing)?;
            if !values_are_consistent(existing, value, atol) {
                return Err(crate::error!(
                    "Conflicting variable fixings for ID={var_id}: \
                     existing={existing}, new={value}"
                ));
            }
            // Same value: nothing to do.
        } else {
            expanded.entries.insert(var_id, value);
            *changed = true;
        }
    }
    Ok(())
}

struct StatePopulationPlan<'a> {
    all: VariableIDSet,
    used: VariableIDSet,
    fixed: Vec<(VariableID, f64)>,
    irrelevant: Vec<(VariableID, Kind, Bound)>,
    dependency: &'a AcyclicAssignments,
}

impl StatePopulationPlan<'_> {
    fn populate(&self, mut state: v1::State, atol: ATol) -> Result<v1::State> {
        let state_ids: VariableIDSet = state.entries.keys().map(|id| (*id).into()).collect();

        let unknown_ids: VariableIDSet = state_ids.difference(&self.all).cloned().collect();
        if !unknown_ids.is_empty() {
            crate::bail!(
                { ?unknown_ids },
                "state contains unknown variable IDs: {unknown_ids:?}",
            );
        }

        let missing_ids: VariableIDSet = self.used.difference(&state_ids).cloned().collect();
        if !missing_ids.is_empty() {
            crate::bail!(
                { ?missing_ids },
                "state is missing required variable IDs: {missing_ids:?}",
            );
        }

        for (&id, &value) in &state.entries {
            ensure_state_value_is_finite(id, value)?;
        }

        // Bound and kind checking is intentionally left to Solution::feasible().
        for (id, value) in &self.fixed {
            ensure_instance_value_is_finite(*id, *value)?;
            use std::collections::hash_map::Entry;
            match state.entries.entry(id.into_inner()) {
                Entry::Occupied(entry) => {
                    let state_value = *entry.get();
                    if !values_are_consistent(state_value, *value, atol) {
                        let instance_value = *value;
                        crate::bail!(
                            { id = ?id, state_value, instance_value },
                            "state value for variable {id:?} is inconsistent with instance (state={state_value}, instance={instance_value})",
                        );
                    }
                }
                Entry::Vacant(entry) => {
                    entry.insert(*value);
                }
            }
        }

        for (id, kind, bound) in &self.irrelevant {
            use std::collections::hash_map::Entry;
            match state.entries.entry(id.into_inner()) {
                Entry::Occupied(_entry) => {}
                Entry::Vacant(entry) => {
                    let value = match kind {
                        Kind::Binary | Kind::Integer | Kind::Continuous => bound.nearest_to_zero(),
                        Kind::SemiInteger | Kind::SemiContinuous => 0.0,
                    };
                    entry.insert(value);
                }
            }
        }

        for (id, f) in self.dependency.evaluation_order_iter() {
            let value = f.evaluate(&state, atol).inspect_err(|e| {
                tracing::error!(?id, error = %e, "failed to evaluate dependent variable");
            })?;
            if !value.is_finite() {
                crate::bail!(
                    { id = ?id, value },
                    "dependent variable {id:?} evaluated to non-finite value: {value}",
                );
            }
            use std::collections::hash_map::Entry;
            match state.entries.entry(id.into_inner()) {
                Entry::Occupied(entry) => {
                    let state_value = *entry.get();
                    if !values_are_consistent(state_value, value, atol) {
                        crate::bail!(
                            { id = ?id, state_value, instance_value = value },
                            "state value for variable {id:?} is inconsistent with instance (state={state_value}, instance={value})",
                        );
                    }
                }
                Entry::Vacant(entry) => {
                    entry.insert(value);
                }
            }
        }

        Ok(state)
    }
}

impl Instance {
    fn state_population_plan(&self) -> StatePopulationPlan<'_> {
        let all: VariableIDSet = self.decision_variables.keys().copied().collect();
        let used = self.used_decision_variable_ids();

        let fixed: Vec<_> = self
            .fixed_decision_variable_values
            .iter()
            .map(|(id, value)| (*id, *value))
            .collect();
        let fixed_ids: VariableIDSet = fixed.iter().map(|(id, _)| *id).collect();
        let dependent_ids: VariableIDSet = self.decision_variable_dependency.keys().collect();
        let relevant: VariableIDSet = used
            .iter()
            .chain(fixed_ids.iter())
            .chain(dependent_ids.iter())
            .copied()
            .collect();

        let irrelevant = self
            .decision_variables
            .iter()
            .filter(|(id, _)| !relevant.contains(id))
            .map(|(id, dv)| (*id, dv.kind(), dv.bound()))
            .collect();

        StatePopulationPlan {
            all,
            used,
            fixed,
            irrelevant,
            dependency: &self.decision_variable_dependency,
        }
    }

    /// Check the state is valid for this instance and populate fixed,
    /// irrelevant, and dependent decision variables.
    ///
    /// Post-condition: the returned state contains exactly this instance's
    /// decision-variable IDs.
    pub fn populate_state(&self, state: v1::State, atol: ATol) -> Result<v1::State> {
        self.state_population_plan().populate(state, atol)
    }
}

impl Evaluate for Instance {
    type Output = crate::Solution;
    type SampledOutput = crate::SampleSet;

    #[tracing::instrument(skip_all)]
    fn evaluate(&self, state: &v1::State, atol: ATol) -> Result<Self::Output> {
        let state = self.populate_state(state.clone(), atol)?;

        let objective = self.objective.evaluate(&state, atol)?;
        let evaluated_constraints = self.constraint_collection.evaluate(&state, atol)?;
        let evaluated_indicator_constraints = self
            .indicator_constraint_collection
            .evaluate(&state, atol)?;
        let evaluated_one_hot_constraints =
            self.one_hot_constraint_collection.evaluate(&state, atol)?;
        let evaluated_sos1_constraints = self.sos1_constraint_collection.evaluate(&state, atol)?;

        let mut decision_variables = BTreeMap::default();
        for (id, dv) in self.decision_variables.iter() {
            let evaluated_dv = evaluate_decision_variable(*id, dv, &state)?;
            decision_variables.insert(*id, evaluated_dv);
        }

        let (evaluated_named_functions, evaluated_named_function_labels) =
            self.named_functions.evaluate(&state, atol)?.into_parts();

        let sense = self.sense();

        // SAFETY: Instance invariants guarantee Solution invariants
        let solution = unsafe {
            crate::Solution::builder()
                .objective(objective)
                .evaluated_constraints_collection(evaluated_constraints)
                .evaluated_indicator_constraints_collection(evaluated_indicator_constraints)
                .evaluated_one_hot_constraints_collection(evaluated_one_hot_constraints)
                .evaluated_sos1_constraints_collection(evaluated_sos1_constraints)
                .evaluated_named_functions(evaluated_named_functions)
                .decision_variables(decision_variables)
                .variable_labels(self.variable_labels.clone())
                .named_function_labels(evaluated_named_function_labels)
                .sense(sense)
                .build_unchecked()?
        };

        Ok(solution)
    }

    #[tracing::instrument(skip_all)]
    fn evaluate_samples(
        &self,
        samples: &crate::Sampled<v1::State>,
        atol: ATol,
    ) -> Result<Self::SampledOutput> {
        // Populate the decision variables in the samples
        let samples = {
            let population = self.state_population_plan();
            let mut samples = samples.clone();
            for state in samples.iter_mut() {
                let taken = std::mem::take(state);
                *state = population.populate(taken, atol)?;
            }
            samples
        };

        let sampled_constraints: crate::constraint_type::SampledCollection<crate::Constraint> =
            self.constraint_collection
                .evaluate_samples(&samples, atol)?;
        let sampled_indicator_constraints: crate::constraint_type::SampledCollection<
            crate::IndicatorConstraint,
        > = self
            .indicator_constraint_collection
            .evaluate_samples(&samples, atol)?;
        let sampled_one_hot_constraints: crate::constraint_type::SampledCollection<
            crate::OneHotConstraint,
        > = self
            .one_hot_constraint_collection
            .evaluate_samples(&samples, atol)?;
        let sampled_sos1_constraints: crate::constraint_type::SampledCollection<
            crate::Sos1Constraint,
        > = self
            .sos1_constraint_collection
            .evaluate_samples(&samples, atol)?;

        // Objective
        let objectives = self.objective().evaluate_samples(&samples, atol)?;

        // Reconstruct decision variable values
        let mut decision_variables = std::collections::BTreeMap::new();
        for (id, dv) in self.decision_variables.iter() {
            let sampled_dv = evaluate_decision_variable_samples(*id, dv, &samples)?;
            decision_variables.insert(*id, sampled_dv);
        }

        let (named_functions, named_function_labels) = self
            .named_functions
            .evaluate_samples(&samples, atol)?
            .into_parts();

        Ok(crate::SampleSet::builder()
            .decision_variables(decision_variables)
            .variable_labels(self.variable_labels.clone())
            .objectives(objectives)
            .constraints_collection(sampled_constraints)
            .indicator_constraints_collection(sampled_indicator_constraints)
            .one_hot_constraints_collection(sampled_one_hot_constraints)
            .sos1_constraints_collection(sampled_sos1_constraints)
            .named_functions(named_functions)
            .named_function_labels(named_function_labels)
            .sense(self.sense)
            .build()?)
    }

    #[tracing::instrument(skip_all)]
    fn partial_evaluate(&mut self, state: &v1::State, atol: ATol) -> Result<()> {
        // Operate on a clone so that any failure leaves `self` unchanged (atomic).
        // Propagation consumes constraints via `self` in `Propagate`, so even a
        // partial failure would otherwise leave the Instance in an inconsistent state.
        let mut working = self.clone();

        // Phase 1: Propagate through special constraints (unit propagation).
        let expanded_state = working.propagate_special_constraints(state, atol)?;

        // Phase 2: Store fixed values in the root-owned table.
        for (id, value) in expanded_state.entries.iter() {
            let var_id = VariableID::from(*id);
            if working.decision_variable_dependency.get(&var_id).is_some() {
                return Err(crate::error!(
                    "Dependent variable (ID={id}) cannot be fixed by partial_evaluate"
                ));
            }
            let Some(dv) = working.decision_variables.get_mut(&var_id) else {
                return Err(crate::error!(
                    "Unknown decision variable (ID={id}) in state."
                ));
            };
            dv.check_value_consistency(var_id, *value, atol)?;
            if let Some(previous_value) = working.fixed_decision_variable_values.get(&var_id) {
                if !values_are_consistent(*previous_value, *value, atol) {
                    return Err(crate::DecisionVariableError::SubstitutedValueOverwrite {
                        id: var_id,
                        previous_value: *previous_value,
                        new_value: *value,
                        atol,
                    }
                    .into());
                }
            } else {
                working
                    .fixed_decision_variable_values
                    .insert(var_id, *value);
            }
        }

        // Phase 3: Regular partial evaluation with expanded state.
        // Special constraint collections are already handled by propagation — not called again.
        working.objective.partial_evaluate(&expanded_state, atol)?;
        working
            .constraint_collection
            .partial_evaluate(&expanded_state, atol)?;
        working
            .named_functions
            .partial_evaluate(&expanded_state, atol)?;
        working
            .decision_variable_dependency
            .partial_evaluate(&expanded_state, atol)?;

        // All operations succeeded; commit changes atomically.
        *self = working;
        Ok(())
    }

    fn required_ids(&self) -> VariableIDSet {
        self.used_decision_variable_ids()
    }
}

impl Instance {
    /// Run unit propagation over special constraint types (OneHot, SOS1, Indicator).
    ///
    /// This is a fixed-point iteration: each constraint is propagated with the current
    /// state, and any additional variable fixings are merged back into the state.
    /// The loop continues until no new fixings are discovered.
    ///
    /// Consumed constraints are moved to the removed set.
    /// Promoted indicator constraints are inserted into the regular constraint collection.
    fn propagate_special_constraints(
        &mut self,
        state: &v1::State,
        atol: ATol,
    ) -> Result<v1::State> {
        let mut expanded = state.clone();
        let mut changed = true;

        let propagation_reason = RemovedReason {
            reason: "ommx.Instance.partial_evaluate.unit_propagation".to_string(),
            parameters: Default::default(),
        };

        while changed {
            changed = false;

            // --- OneHot constraints ---
            self.one_hot_constraint_collection.rewrite_active(
                |_, oh, _| -> crate::Result<ActiveConstraintUpdate<crate::OneHotConstraint>> {
                    let (outcome, additional) = oh.propagate(&expanded, atol)?;
                    merge_state(&mut expanded, additional, atol, &mut changed)?;
                    match outcome {
                        PropagateOutcome::Active(oh) => Ok(ActiveConstraintUpdate::Active(oh)),
                        PropagateOutcome::Consumed(oh) => Ok(ActiveConstraintUpdate::Removed {
                            constraint: oh,
                            reason: propagation_reason.clone(),
                        }),
                        PropagateOutcome::Transformed { new, .. } => match new {},
                    }
                },
            )?;

            // --- SOS1 constraints ---
            self.sos1_constraint_collection.rewrite_active(
                |_, sos1, _| -> crate::Result<ActiveConstraintUpdate<crate::Sos1Constraint>> {
                    let (outcome, additional) = sos1.propagate(&expanded, atol)?;
                    merge_state(&mut expanded, additional, atol, &mut changed)?;
                    match outcome {
                        PropagateOutcome::Active(sos1) => Ok(ActiveConstraintUpdate::Active(sos1)),
                        PropagateOutcome::Consumed(sos1) => Ok(ActiveConstraintUpdate::Removed {
                            constraint: sos1,
                            reason: propagation_reason.clone(),
                        }),
                        PropagateOutcome::Transformed { new, .. } => match new {},
                    }
                },
            )?;

            // --- Indicator constraints ---
            let mut promoted_constraints = Vec::new();
            self.indicator_constraint_collection
                .rewrite_active(|id, ic, context| -> crate::Result<ActiveConstraintUpdate<crate::IndicatorConstraint>> {
                    let (outcome, additional) = ic.propagate(&expanded, atol)?;
                    merge_state(&mut expanded, additional, atol, &mut changed)?;
                    match outcome {
                        PropagateOutcome::Active(ic) => Ok(ActiveConstraintUpdate::Active(ic)),
                        PropagateOutcome::Consumed(ic) => Ok(ActiveConstraintUpdate::Removed {
                            constraint: ic,
                            reason: propagation_reason.clone(),
                        }),
                        PropagateOutcome::Transformed {
                            original,
                            new: constraint,
                        } => {
                            // Indicator=1 → promote inner constraint to regular constraint.
                            // Carry over the indicator's context into the regular collection's
                            // store and record the promotion in provenance.
                            let mut new_context = context.collect_for(id);
                            new_context
                                .provenance
                                .push(crate::constraint::Provenance::IndicatorConstraint(id));
                            promoted_constraints.push((constraint, new_context));
                            Ok(ActiveConstraintUpdate::Removed {
                                constraint: original,
                                reason: propagation_reason.clone(),
                            })
                        }
                    }
                })?;
            for (constraint, context) in promoted_constraints {
                let id = self.constraint_collection.unused_id();
                self.constraint_collection
                    .insert_with(id, constraint, context)?;
            }
        }

        Ok(expanded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::random::arbitrary_split_state;
    use crate::{coeff, linear};
    use ::approx::AbsDiffEq;
    use proptest::prelude::*;
    use std::collections::HashMap;

    proptest! {
        #[test]
        fn test_evaluate_instance(
            (instance, state) in Instance::arbitrary()
                .prop_flat_map(|instance| {
                    let state = instance.arbitrary_state();
                    (Just(instance), state)
                })
        ) {
            let solution = instance.evaluate(&state, ATol::default()).unwrap();
            // Must be populated
            let ids: VariableIDSet = solution.state().entries.keys().map(|id| VariableID::from(*id)).collect();
            let all: VariableIDSet = instance.decision_variables().keys().copied().collect();
            prop_assert_eq!(ids, all);
        }

        #[test]
        fn partial_evaluate(
            (mut instance, state, (u, v)) in Instance::arbitrary()
                .prop_flat_map(|instance| {
                    let state = instance.arbitrary_state();
                    (Just(instance), state).prop_flat_map(|(instance, state)| {
                        let split = arbitrary_split_state(&state);
                        (Just(instance), Just(state), split)
                    })
                })
        ) {
            let s1 = instance.evaluate(&state, ATol::default()).unwrap();
            instance.partial_evaluate(&u, ATol::default()).unwrap();
            let s2 = instance.evaluate(&v, ATol::default()).unwrap();
            prop_assert!(s1.state().abs_diff_eq(&s2.state(), ATol::default()));
        }
    }

    #[test]
    fn test_populate_state_rejects_non_finite_fixed_value_from_state() {
        let decision_variables = BTreeMap::from([
            (VariableID::from(1), crate::DecisionVariable::continuous()),
            (VariableID::from(2), crate::DecisionVariable::continuous()),
        ]);
        let instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(1)))
            .decision_variables(decision_variables)
            .fixed_decision_variable_values(BTreeMap::from([(VariableID::from(2), 3.0)]))
            .constraints(BTreeMap::new())
            .build()
            .unwrap();
        let state = v1::State::from(HashMap::from([(1, 1.0), (2, f64::NAN)]));

        let err = instance.populate_state(state, ATol::default()).unwrap_err();
        assert!(err.to_string().contains("must be finite"));
    }

    #[test]
    fn test_partial_evaluate_accepts_existing_fixed_value_at_atol_boundary() {
        let decision_variables = BTreeMap::from([
            (VariableID::from(1), crate::DecisionVariable::continuous()),
            (VariableID::from(2), crate::DecisionVariable::continuous()),
        ]);
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(1)))
            .decision_variables(decision_variables)
            .fixed_decision_variable_values(BTreeMap::from([(VariableID::from(2), 0.0)]))
            .constraints(BTreeMap::new())
            .build()
            .unwrap();

        let atol = ATol::default();
        let state = v1::State::from(HashMap::from([(2, *atol)]));
        instance.partial_evaluate(&state, atol).unwrap();

        assert_eq!(
            instance.fixed_decision_variable_value(VariableID::from(2)),
            Some(0.0)
        );
    }

    #[test]
    fn test_evaluate_samples_preserves_sample_groups_for_decision_variables() {
        let instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(1)))
            .decision_variables(BTreeMap::from([(
                VariableID::from(1),
                crate::DecisionVariable::continuous(),
            )]))
            .constraints(BTreeMap::new())
            .build()
            .unwrap();
        let samples = crate::Sampled::new(
            [
                vec![crate::SampleID::from(0)],
                vec![crate::SampleID::from(1)],
                vec![crate::SampleID::from(2)],
            ],
            [
                v1::State::from(HashMap::from([(1, 2.0)])),
                v1::State::from(HashMap::from([(1, 7.0)])),
                v1::State::from(HashMap::from([(1, 2.0)])),
            ],
        )
        .unwrap();

        let sample_set = instance
            .evaluate_samples(&samples, ATol::default())
            .unwrap();
        let sampled = sample_set
            .decision_variables()
            .get(&VariableID::from(1))
            .unwrap()
            .samples();

        assert_eq!(sampled.get(crate::SampleID::from(0)), Some(&2.0));
        assert_eq!(sampled.get(crate::SampleID::from(1)), Some(&7.0));
        assert_eq!(sampled.get(crate::SampleID::from(2)), Some(&2.0));

        let chunks = sampled.clone().chunk();
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].0, 2.0);
        assert_eq!(chunks[0].1.len(), 2);
        assert!(chunks[0].1.contains(&crate::SampleID::from(0)));
        assert!(chunks[0].1.contains(&crate::SampleID::from(2)));
        assert_eq!(chunks[1].0, 7.0);
        assert_eq!(chunks[1].1.len(), 1);
        assert!(chunks[1].1.contains(&crate::SampleID::from(1)));
    }

    #[test]
    fn test_partial_evaluate_rejects_dependent_variable_fixing() {
        let decision_variables = BTreeMap::from([
            (VariableID::from(1), crate::DecisionVariable::continuous()),
            (VariableID::from(10), crate::DecisionVariable::continuous()),
        ]);
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(1)))
            .decision_variables(decision_variables)
            .constraints(BTreeMap::new())
            .decision_variable_dependency(crate::assign! {
                10 <- coeff!(2.0) * linear!(1)
            })
            .build()
            .unwrap();

        let state = v1::State::from(HashMap::from([(1, 2.0), (10, 4.0)]));
        let err = instance
            .partial_evaluate(&state, ATol::default())
            .unwrap_err();

        assert!(
            err.to_string()
                .contains("Dependent variable (ID=10) cannot be fixed"),
            "unexpected error: {err}"
        );
        assert_eq!(
            instance.fixed_decision_variable_value(VariableID::from(10)),
            None
        );
        assert!(instance
            .decision_variable_dependency
            .get(&VariableID::from(10))
            .is_some());
    }

    #[test]
    fn test_populate_state_rejects_non_finite_existing_dependent_value() {
        let decision_variables = BTreeMap::from([
            (VariableID::from(1), crate::DecisionVariable::continuous()),
            (VariableID::from(10), crate::DecisionVariable::continuous()),
        ]);
        let mut instance = Instance::new(
            Sense::Minimize,
            Function::from(linear!(1)),
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();
        instance.decision_variable_dependency = crate::assign! {
            10 <- linear!(1)
        };
        let state = v1::State::from(HashMap::from([(1, 1.0), (10, f64::INFINITY)]));

        let err = instance.populate_state(state, ATol::default()).unwrap_err();
        assert!(err.to_string().contains("must be finite"));
    }

    #[test]
    fn test_populate_state_rejects_non_finite_dependent_evaluation() {
        let decision_variables = BTreeMap::from([
            (VariableID::from(1), crate::DecisionVariable::continuous()),
            (VariableID::from(10), crate::DecisionVariable::continuous()),
        ]);
        let mut instance = Instance::new(
            Sense::Minimize,
            Function::from(linear!(1)),
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();
        instance.decision_variable_dependency = crate::assign! {
            10 <- coeff!(f64::MAX) * linear!(1)
        };
        let state = v1::State::from(HashMap::from([(1, f64::MAX)]));

        let err = instance.populate_state(state, ATol::default()).unwrap_err();
        assert!(err.to_string().contains("evaluated to non-finite value"));
    }

    /// Test that named functions can reference fixed, dependent, and irrelevant variables
    #[test]
    fn test_evaluate_named_function_with_fixed_dependent_irrelevant_variables() {
        use crate::{DecisionVariable, NamedFunction, NamedFunctionID};
        use maplit::btreemap;

        // Create decision variables:
        // x1 (id=1): used in objective
        // x2 (id=2): fixed variable (value = 3.0)
        // x3 (id=3): dependent on x4 (x3 = 2 * x4)
        // x4 (id=4): irrelevant (not used in objective/constraints)
        // x5 (id=5): only used in named_functions

        let x1 = DecisionVariable::continuous();
        let x2 = DecisionVariable::continuous();
        let x3 = DecisionVariable::continuous();
        let x4 = DecisionVariable::continuous();
        let x5 = DecisionVariable::continuous();

        let decision_variables = btreemap! {
            VariableID::from(1) => x1,
            VariableID::from(2) => x2,
            VariableID::from(3) => x3,
            VariableID::from(4) => x4,
            VariableID::from(5) => x5,
        };

        // Objective: minimize x1 (only x1 is "used")
        let objective = Function::from(linear!(1));

        // Create instance with dependency: x3 = 2 * x4
        let decision_variable_dependency = crate::AcyclicAssignments::new(vec![(
            VariableID::from(3),
            Function::from(coeff!(2.0) * linear!(4)),
        )])
        .unwrap();

        // Named function: f = x2 + x3 + x4 + x5
        // This references:
        // - x2: fixed variable
        // - x3: dependent variable
        // - x4: irrelevant variable
        // - x5: only used in named function
        let named_function = NamedFunction {
            function: Function::from(
                (((linear!(2) + linear!(3)).unwrap() + linear!(4)).unwrap() + linear!(5)).unwrap(),
            ),
        };

        let named_functions = btreemap! {
            NamedFunctionID::from(1) => named_function,
        };

        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(objective)
            .decision_variables(decision_variables)
            .fixed_decision_variable_values(btreemap! {
                VariableID::from(2) => 3.0,
            })
            .constraints(BTreeMap::new()) // No constraints
            .named_functions(named_functions)
            .build()
            .unwrap();
        instance.decision_variable_dependency = decision_variable_dependency;

        // Verify the usage: x1 is used, x2 is fixed, x3 is dependent,
        // x4 and x5 should be irrelevant (named_functions don't contribute to "used")
        let usage = instance.decision_variable_usage();
        assert!(usage.used().contains(&VariableID::from(1)));
        assert!(usage.fixed().contains_key(&VariableID::from(2)));
        assert!(usage.dependent().contains(&VariableID::from(3)));
        assert!(usage.irrelevant().contains(&VariableID::from(4)));
        assert!(usage.irrelevant().contains(&VariableID::from(5)));

        // Create state: x1=1.0, x4=2.0, x5=10.0
        // x2 is fixed to 3.0
        // x3 is dependent: x3 = 2 * x4 = 4.0
        let state = v1::State::from(HashMap::from([(1, 1.0), (4, 2.0), (5, 10.0)]));

        let solution = instance.evaluate(&state, ATol::default()).unwrap();

        // Check objective value
        assert_eq!(*solution.objective(), 1.0);

        // Check named function evaluation
        // f = x2 + x3 + x4 + x5 = 3.0 + 4.0 + 2.0 + 10.0 = 19.0
        let evaluated_nf = solution
            .evaluated_named_functions()
            .get(&NamedFunctionID::from(1))
            .unwrap();
        assert_eq!(evaluated_nf.evaluated_value(), 19.0);

        // Check used_decision_variable_ids of the evaluated named function
        // It contains the variable IDs referenced in the named function's expression,
        // which are x2, x3, x4, x5 (dependency substitution is done at the state level,
        // not at the expression level)
        let used_ids = evaluated_nf.used_decision_variable_ids();
        assert!(used_ids.contains(&VariableID::from(2)));
        assert!(used_ids.contains(&VariableID::from(3)));
        assert!(used_ids.contains(&VariableID::from(4)));
        assert!(used_ids.contains(&VariableID::from(5)));
        // x1 is not used in the named function
        assert!(!used_ids.contains(&VariableID::from(1)));
    }

    // === Unit propagation integration tests ===

    #[test]
    fn test_partial_evaluate_one_hot_propagation() {
        use crate::{DecisionVariable, OneHotConstraint, OneHotConstraintID};
        use maplit::btreemap;

        // Binary variables x1, x2, x3 with OneHot{x1, x2, x3}
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(),
            VariableID::from(2) => DecisionVariable::binary(),
            VariableID::from(3) => DecisionVariable::binary(),
        };
        let objective = Function::from(((linear!(1) + linear!(2)).unwrap() + linear!(3)).unwrap());

        let mut instance = Instance::new(
            Sense::Minimize,
            objective,
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();

        let oh =
            OneHotConstraint::new([1, 2, 3].into_iter().map(VariableID::from).collect()).unwrap();
        instance
            .one_hot_constraint_collection
            .insert_with(
                OneHotConstraintID::from(1),
                oh,
                crate::ConstraintContext::default(),
            )
            .unwrap();

        // Fix x2 = 1 → OneHot propagation should fix x1=0, x3=0
        let state = v1::State::from(HashMap::from([(2, 1.0)]));
        instance.partial_evaluate(&state, ATol::default()).unwrap();

        // All three variables should be substituted
        assert_eq!(
            instance.fixed_decision_variable_value(VariableID::from(1)),
            Some(0.0)
        );
        assert_eq!(
            instance.fixed_decision_variable_value(VariableID::from(2)),
            Some(1.0)
        );
        assert_eq!(
            instance.fixed_decision_variable_value(VariableID::from(3)),
            Some(0.0)
        );

        // OneHot constraint should be consumed (moved to removed)
        assert!(instance.one_hot_constraint_collection.active().is_empty());
        assert_eq!(instance.one_hot_constraint_collection.removed().len(), 1);
    }

    #[test]
    fn test_partial_evaluate_one_hot_unit_propagation() {
        use crate::{DecisionVariable, OneHotConstraint, OneHotConstraintID};
        use maplit::btreemap;

        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(),
            VariableID::from(2) => DecisionVariable::binary(),
            VariableID::from(3) => DecisionVariable::binary(),
        };
        let objective = Function::from(((linear!(1) + linear!(2)).unwrap() + linear!(3)).unwrap());

        let mut instance = Instance::new(
            Sense::Minimize,
            objective,
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();

        let oh =
            OneHotConstraint::new([1, 2, 3].into_iter().map(VariableID::from).collect()).unwrap();
        instance
            .one_hot_constraint_collection
            .insert_with(
                OneHotConstraintID::from(1),
                oh,
                crate::ConstraintContext::default(),
            )
            .unwrap();

        // Fix x1=0, x2=0 → unit propagation: x3 must be 1
        let state = v1::State::from(HashMap::from([(1, 0.0), (2, 0.0)]));
        instance.partial_evaluate(&state, ATol::default()).unwrap();

        assert_eq!(
            instance.fixed_decision_variable_value(VariableID::from(3)),
            Some(1.0)
        );
    }

    #[test]
    fn test_partial_evaluate_cascade_one_hot_sos1() {
        use crate::{
            DecisionVariable, OneHotConstraint, OneHotConstraintID, Sos1Constraint,
            Sos1ConstraintID,
        };
        use maplit::btreemap;

        // x1, x2 in OneHot; x2, x3 in SOS1
        // Fix x1=1 → OneHot propagates x2=0 → SOS1 shrinks (x2 removed)
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(),
            VariableID::from(2) => DecisionVariable::binary(),
            VariableID::from(3) => DecisionVariable::continuous(),
        };
        let objective = Function::from(((linear!(1) + linear!(2)).unwrap() + linear!(3)).unwrap());

        let mut instance = Instance::new(
            Sense::Minimize,
            objective,
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();

        let oh = OneHotConstraint::new([1, 2].into_iter().map(VariableID::from).collect()).unwrap();
        instance
            .one_hot_constraint_collection
            .insert_with(
                OneHotConstraintID::from(1),
                oh,
                crate::ConstraintContext::default(),
            )
            .unwrap();

        let sos1 = Sos1Constraint::new([2, 3].into_iter().map(VariableID::from).collect()).unwrap();
        instance
            .sos1_constraint_collection
            .insert_with(
                Sos1ConstraintID::from(1),
                sos1,
                crate::ConstraintContext::default(),
            )
            .unwrap();

        // Fix x1=1 → OneHot: x2=0 → SOS1{x2,x3} shrinks to SOS1{x3}
        let state = v1::State::from(HashMap::from([(1, 1.0)]));
        instance.partial_evaluate(&state, ATol::default()).unwrap();

        // x2 should be fixed to 0 by propagation
        assert_eq!(
            instance.fixed_decision_variable_value(VariableID::from(2)),
            Some(0.0)
        );

        // OneHot consumed, SOS1 shrunk to just x3
        assert!(instance.one_hot_constraint_collection.active().is_empty());
        let sos1_active = instance.sos1_constraint_collection.active();
        assert_eq!(sos1_active.len(), 1);
        let remaining_sos1 = sos1_active.values().next().unwrap();
        assert_eq!(remaining_sos1.variables.len(), 1);
        assert!(remaining_sos1.variables.contains(&VariableID::from(3)));
    }

    #[test]
    fn test_partial_evaluate_indicator_promotion() {
        use crate::{constraint::Equality, DecisionVariable, IndicatorConstraintID};
        use maplit::btreemap;

        // x10 (indicator), x1, x2 (function variables)
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::continuous(),
            VariableID::from(2) => DecisionVariable::continuous(),
            VariableID::from(10) => DecisionVariable::binary(),
        };
        let objective = Function::from(linear!(1) + linear!(2));

        let mut indicator_constraints = BTreeMap::new();
        indicator_constraints.insert(
            IndicatorConstraintID::from(100),
            crate::IndicatorConstraint::new(
                VariableID::from(10),
                Equality::LessThanOrEqualToZero,
                Function::from(((linear!(1) + linear!(2)).unwrap() + coeff!(-5.0)).unwrap()),
            ),
        );

        let instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(objective)
            .decision_variables(decision_variables)
            .constraints(BTreeMap::new())
            .indicator_constraints(indicator_constraints)
            .build()
            .unwrap();

        let mut instance = instance;

        // Fix x10=1 → indicator promoted to regular constraint
        let state = v1::State::from(HashMap::from([(10, 1.0)]));
        instance.partial_evaluate(&state, ATol::default()).unwrap();

        // Indicator constraint should be removed
        assert!(instance.indicator_constraint_collection.active().is_empty());
        assert_eq!(instance.indicator_constraint_collection.removed().len(), 1);

        // A new regular constraint should be added, and its provenance
        // should reference the original IndicatorConstraintID so that the
        // transformation lineage is preserved.
        assert_eq!(instance.constraint_collection.active().len(), 1);
        let (cid, _promoted) = instance
            .constraint_collection
            .active()
            .iter()
            .next()
            .unwrap();
        assert_eq!(
            instance.constraint_collection.context().provenance(*cid),
            &[crate::constraint::Provenance::IndicatorConstraint(
                IndicatorConstraintID::from(100)
            )]
        );
    }

    #[test]
    fn test_partial_evaluate_indicator_removed() {
        use crate::{constraint::Equality, DecisionVariable, IndicatorConstraintID};
        use maplit::btreemap;

        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::continuous(),
            VariableID::from(10) => DecisionVariable::binary(),
        };
        let objective = Function::from(linear!(1));

        let mut indicator_constraints = BTreeMap::new();
        indicator_constraints.insert(
            IndicatorConstraintID::from(1),
            crate::IndicatorConstraint::new(
                VariableID::from(10),
                Equality::LessThanOrEqualToZero,
                Function::from(linear!(1) + coeff!(-5.0)),
            ),
        );

        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(objective)
            .decision_variables(decision_variables)
            .constraints(BTreeMap::new())
            .indicator_constraints(indicator_constraints)
            .build()
            .unwrap();

        // Fix x10=0 → indicator removed (vacuously satisfied)
        let state = v1::State::from(HashMap::from([(10, 0.0)]));
        instance.partial_evaluate(&state, ATol::default()).unwrap();

        assert!(instance.indicator_constraint_collection.active().is_empty());
        assert_eq!(instance.indicator_constraint_collection.removed().len(), 1);
        // No new regular constraint should be added
        assert!(instance.constraint_collection.active().is_empty());
    }
}
