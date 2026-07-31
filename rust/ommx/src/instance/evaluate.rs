use super::*;
use crate::Result;
use crate::{
    constraint::RemovedReason, ATol, Bound, DecisionVariableError, Evaluate,
    InconsistentDependentValue, Kind, MissingStateEntries, Propagate, PropagateOutcome,
    UnknownStateEntries, UnverifiableDependentAssertion, VariableIDSet,
};
use std::collections::BTreeMap;

fn ensure_state_value_is_finite(var_id: u64, value: f64) -> Result<()> {
    if !value.is_finite() {
        return Err(DecisionVariableError::NonFiniteValue {
            id: var_id.into(),
            value,
        }
        .into());
    }
    Ok(())
}

fn ensure_internal_state_value_is_finite(var_id: u64, value: f64) -> Result<()> {
    if !value.is_finite() {
        crate::bail!(
            { var_id, value },
            "state value for variable ID={var_id} must be finite (value={value})",
        );
    }
    Ok(())
}

fn invalid_propagated_value(
    id: VariableID,
    value: f64,
    error: impl std::fmt::Display,
) -> crate::Error {
    crate::error!(
        { id = ?id, value, cause = %error },
        "special-constraint propagation produced an invalid value for decision variable {id:?}: {error}",
    )
}

fn normalize_dependency_partial_evaluation_error(
    id: VariableID,
    error: crate::Error,
) -> crate::Error {
    if !error.is::<crate::CoefficientError>() {
        return error;
    }

    // Direct function and polynomial APIs intentionally expose the
    // coefficient-domain signal. During Instance-owned dependency
    // normalization, the same coefficient arithmetic is an implementation
    // detail, so retaining the signal would expose a false recovery path.
    crate::error!(
        { id = ?id, cause = %error },
        "failed to normalize dependent variable {id:?}: {error:#}",
    )
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

fn fixed_values_state(fixed_values: &BTreeMap<VariableID, f64>) -> v1::State {
    v1::State::from(
        fixed_values
            .iter()
            .map(|(id, value)| (id.into_inner(), *value))
            .collect::<std::collections::HashMap<_, _>>(),
    )
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
        ensure_internal_state_value_is_finite(var_id, value)?;
        if let Some(&existing) = expanded.entries.get(&var_id) {
            ensure_internal_state_value_is_finite(var_id, existing)?;
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

enum PartialEvaluatePlan {
    FixedValuesOnly {
        fixed_values: BTreeMap<VariableID, f64>,
    },
    RegularReplacement {
        fixed_values: BTreeMap<VariableID, f64>,
        objective: Function,
        active_constraint_replacements: BTreeMap<ConstraintID, Constraint>,
        named_function_replacements: BTreeMap<NamedFunctionID, NamedFunction>,
    },
}

impl PartialEvaluatePlan {
    fn prepare(instance: &Instance, state: &v1::State, atol: ATol) -> Result<Option<Self>> {
        if Self::supports_fixed_values_only_fast_path(instance) {
            return Ok(Some(Self::FixedValuesOnly {
                fixed_values: Self::prepare_fixed_values(instance, state, atol)?,
            }));
        }

        if !Self::supports_regular_replacement_shape(instance) {
            return Ok(None);
        }

        let fixed_values = Self::prepare_fixed_values(instance, state, atol)?;
        let evaluation_state = Self::evaluation_state(instance, &fixed_values);

        let mut objective = instance.objective.clone();
        objective.partial_evaluate(&evaluation_state, atol)?;

        let mut active_constraint_replacements = BTreeMap::new();
        for (&id, constraint) in instance.constraint_collection.active() {
            let mut constraint = constraint.clone();
            constraint.partial_evaluate(&evaluation_state, atol)?;
            active_constraint_replacements.insert(id, constraint);
        }

        let mut named_function_replacements = BTreeMap::new();
        for (&id, named_function) in instance.named_functions.entries() {
            let mut named_function = named_function.clone();
            named_function.partial_evaluate(&evaluation_state, atol)?;
            named_function_replacements.insert(id, named_function);
        }

        Ok(Some(Self::RegularReplacement {
            fixed_values,
            objective,
            active_constraint_replacements,
            named_function_replacements,
        }))
    }

    fn prepare_fixed_values_only(
        instance: &Instance,
        state: &v1::State,
        atol: ATol,
    ) -> Result<Option<Self>> {
        if !Self::supports_fixed_values_only_fast_path(instance) {
            return Ok(None);
        }

        Ok(Some(Self::FixedValuesOnly {
            fixed_values: Self::prepare_fixed_values(instance, state, atol)?,
        }))
    }

    fn has_no_active_special_constraints(instance: &Instance) -> bool {
        instance.indicator_constraint_collection.active().is_empty()
            && instance.one_hot_constraint_collection.active().is_empty()
            && instance.sos1_constraint_collection.active().is_empty()
    }

    fn supports_fixed_values_only_fast_path(instance: &Instance) -> bool {
        instance.objective.required_ids().is_empty()
            && instance.constraint_collection.active().is_empty()
            && Self::has_no_active_special_constraints(instance)
            && instance.decision_variable_dependency.is_empty()
            && instance.named_functions.required_ids().is_empty()
    }

    fn supports_regular_replacement_shape(instance: &Instance) -> bool {
        Self::has_no_active_special_constraints(instance)
            && instance
                .indicator_constraint_collection
                .removed()
                .is_empty()
            && instance.one_hot_constraint_collection.removed().is_empty()
            && instance.sos1_constraint_collection.removed().is_empty()
            && instance.decision_variable_dependency.is_empty()
            && !instance.constraint_collection.removed().is_empty()
    }

    fn prepare_fixed_values(
        instance: &Instance,
        state: &v1::State,
        atol: ATol,
    ) -> Result<BTreeMap<VariableID, f64>> {
        Self::validate_state(instance, state, atol)?;
        Ok(state
            .entries
            .iter()
            .map(|(&id, &value)| (VariableID::from(id), value))
            .collect())
    }

    fn validate_state(instance: &Instance, state: &v1::State, atol: ATol) -> Result<()> {
        let unknown_ids: VariableIDSet = state
            .entries
            .keys()
            .map(|&id| VariableID::from(id))
            .filter(|id| instance.decision_variables.get(id).is_none())
            .collect();
        if !unknown_ids.is_empty() {
            return Err(UnknownStateEntries { ids: unknown_ids }.into());
        }

        for (&id, &value) in &state.entries {
            ensure_state_value_is_finite(id, value)?;
            let var_id = VariableID::from(id);
            let dv = instance
                .decision_variables
                .get(&var_id)
                .expect("state variable IDs were validated above");
            dv.check_value_consistency(var_id, value, atol)?;
            if let Some(previous_value) = instance.decision_variables.fixed_value(var_id) {
                if !values_are_consistent(previous_value, value, atol) {
                    return Err(DecisionVariableError::SubstitutedValueOverwrite {
                        id: var_id,
                        previous_value,
                        new_value: value,
                        atol,
                    }
                    .into());
                }
            }
        }
        Ok(())
    }

    fn evaluation_state(
        instance: &Instance,
        fixed_values: &BTreeMap<VariableID, f64>,
    ) -> v1::State {
        let mut merged = instance.fixed_decision_variable_values().clone();
        for (&id, &value) in fixed_values {
            merged.entry(id).or_insert(value);
        }
        fixed_values_state(&merged)
    }
}

impl StatePopulationPlan<'_> {
    fn populate(&self, mut state: v1::State, atol: ATol) -> Result<v1::State> {
        let state_ids: VariableIDSet = state.entries.keys().map(|id| (*id).into()).collect();

        let unknown_ids: VariableIDSet = state_ids.difference(&self.all).cloned().collect();
        if !unknown_ids.is_empty() {
            return Err(UnknownStateEntries { ids: unknown_ids }.into());
        }

        let missing_ids: VariableIDSet = self.used.difference(&state_ids).cloned().collect();
        if !missing_ids.is_empty() {
            return Err(MissingStateEntries { ids: missing_ids }.into());
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
                        return Err(DecisionVariableError::SubstitutedValueOverwrite {
                            id: *id,
                            previous_value: *value,
                            new_value: state_value,
                            atol,
                        }
                        .into());
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
                        return Err(InconsistentDependentValue {
                            id,
                            state_value,
                            dependency_value: value,
                        }
                        .into());
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
            .fixed_decision_variable_values()
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

    /// Partially evaluate this instance by consuming it.
    ///
    /// This applies the same mathematical operation as
    /// [`Evaluate::partial_evaluate`], but returns the rewritten instance instead
    /// of mutating a borrowed one. Because the original instance is consumed, an
    /// error does not need to preserve an observable rollback state.
    pub fn into_partial_evaluated(mut self, state: &v1::State, atol: ATol) -> Result<Self> {
        self.partial_evaluate_in_place(state, atol)?;
        Ok(self)
    }

    fn partial_evaluate_in_place(&mut self, state: &v1::State, atol: ATol) -> Result<()> {
        if let Some(plan) = PartialEvaluatePlan::prepare_fixed_values_only(self, state, atol)? {
            self.commit_partial_evaluate_plan(plan, atol);
            return Ok(());
        }

        self.partial_evaluate_fallback_in_place(state, atol)
    }

    fn commit_partial_evaluate_plan(&mut self, plan: PartialEvaluatePlan, atol: ATol) {
        match plan {
            PartialEvaluatePlan::FixedValuesOnly { fixed_values } => {
                self.decision_variables
                    .merge_validated_fixed_values(fixed_values, atol);
            }
            PartialEvaluatePlan::RegularReplacement {
                fixed_values,
                objective,
                active_constraint_replacements,
                named_function_replacements,
            } => {
                self.decision_variables
                    .merge_validated_fixed_values(fixed_values, atol);
                self.objective = objective;
                self.constraint_collection
                    .replace_active_rows(active_constraint_replacements)
                    .expect(
                        "partial-evaluate plan prepared replacements from active constraint IDs",
                    );
                self.named_functions
                    .replace_rows(named_function_replacements)
                    .expect("partial-evaluate plan prepared replacements from named-function IDs");
            }
        }
    }

    fn partial_evaluate_fallback_in_place(&mut self, state: &v1::State, atol: ATol) -> Result<()> {
        // The input state belongs to Instance validation. Validate it before
        // special-constraint propagation so propagation errors only describe
        // failures in the derived state or the constraints themselves.
        PartialEvaluatePlan::validate_state(self, state, atol)?;

        // Phase 1: Propagate through special constraints (unit propagation).
        let expanded_state = self.propagate_special_constraints(state, atol)?;

        // Phase 2: Store fixed values in the decision-variable table. Values for
        // dependent keys are consistency assertions checked during dependency
        // normalization below; they are not direct writes to fixed values.
        let mut dependent_assertions = BTreeMap::new();
        for (id, value) in expanded_state.entries.iter() {
            let var_id = VariableID::from(*id);
            let Some(dv) = self.decision_variables.get(&var_id) else {
                return Err(crate::error!(
                    "special-constraint propagation produced an unknown decision variable (ID={id})"
                ));
            };
            dv.check_value_consistency(var_id, *value, atol)
                .map_err(|error| invalid_propagated_value(var_id, *value, error))?;
            if self.decision_variable_dependency.get(&var_id).is_some() {
                dependent_assertions.insert(var_id, *value);
            } else {
                self.decision_variables
                    .ensure_fixed_value(var_id, *value, atol)
                    .map_err(|error| invalid_propagated_value(var_id, *value, error))?;
            }
        }

        let normalized_state = self.normalize_constant_dependencies(dependent_assertions, atol)?;

        // Phase 3: Regular partial evaluation with normalized fixed values.
        // Special constraint collections are already handled by propagation — not called again.
        self.objective.partial_evaluate(&normalized_state, atol)?;
        self.constraint_collection
            .partial_evaluate(&normalized_state, atol)?;
        self.named_functions
            .partial_evaluate(&normalized_state, atol)?;

        Ok(())
    }

    fn normalize_constant_dependencies(
        &mut self,
        mut assertions: BTreeMap<VariableID, f64>,
        atol: ATol,
    ) -> Result<v1::State> {
        let mut evaluation_state = fixed_values_state(self.fixed_decision_variable_values());
        let mut remaining_assignments = Vec::new();

        for (id, function) in self.decision_variable_dependency.evaluation_order_iter() {
            let mut function = function.clone();
            function
                .partial_evaluate(&evaluation_state, atol)
                .map_err(|error| normalize_dependency_partial_evaluation_error(id, error))?;
            let required_ids = function.required_ids();

            if required_ids.is_empty() {
                let value = function.evaluate(&v1::State::default(), atol)?;
                if !value.is_finite() {
                    crate::bail!(
                        { id = ?id, value },
                        "dependent variable {id:?} evaluated to non-finite value: {value}",
                    );
                }
                if let Some(asserted_value) = assertions.remove(&id) {
                    if !values_are_consistent(asserted_value, value, atol) {
                        return Err(InconsistentDependentValue {
                            id,
                            state_value: asserted_value,
                            dependency_value: value,
                        }
                        .into());
                    }
                }
                let dv = self.decision_variables.get(&id).ok_or_else(|| {
                    crate::error!(
                        "Variable ID {id:?} in decision_variable_dependency is not in decision_variables"
                    )
                })?;
                dv.check_value_consistency(id, value, atol)?;
                self.decision_variables
                    .ensure_fixed_value(id, value, atol)?;
                evaluation_state.entries.insert(id.into_inner(), value);
            } else {
                if assertions.remove(&id).is_some() {
                    return Err(UnverifiableDependentAssertion { id, required_ids }.into());
                }
                remaining_assignments.push((id, function));
            }
        }

        self.decision_variable_dependency = AcyclicAssignments::new(remaining_assignments)?;
        Ok(evaluation_state)
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

        let evaluated_named_functions = self.named_functions.evaluate(&state, atol)?;

        let sense = self.sense();

        // SAFETY: Instance invariants guarantee Solution invariants
        let solution = unsafe {
            crate::Solution::builder()
                .objective(objective)
                .evaluated_constraints_collection(evaluated_constraints)
                .evaluated_indicator_constraints_collection(evaluated_indicator_constraints)
                .evaluated_one_hot_constraints_collection(evaluated_one_hot_constraints)
                .evaluated_sos1_constraints_collection(evaluated_sos1_constraints)
                .evaluated_named_function_table(evaluated_named_functions)
                .decision_variables(decision_variables)
                .variable_labels(self.variable_labels().clone())
                .sense(sense)
                .feasibility_atol(atol)
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

        let named_functions = self.named_functions.evaluate_samples(&samples, atol)?;

        Ok(crate::SampleSet::builder()
            .decision_variables(decision_variables)
            .variable_labels(self.variable_labels().clone())
            .objectives(objectives)
            .constraints_collection(sampled_constraints)
            .indicator_constraints_collection(sampled_indicator_constraints)
            .one_hot_constraints_collection(sampled_one_hot_constraints)
            .sos1_constraints_collection(sampled_sos1_constraints)
            .named_function_table(named_functions)
            .sense(self.sense)
            .feasibility_atol(atol)
            .build()?)
    }

    #[tracing::instrument(skip_all)]
    fn partial_evaluate(&mut self, state: &v1::State, atol: ATol) -> Result<()> {
        if let Some(plan) = PartialEvaluatePlan::prepare(self, state, atol)? {
            self.commit_partial_evaluate_plan(plan, atol);
            return Ok(());
        }

        // Operate on a clone so that any failure leaves `self` unchanged (atomic).
        // Propagation consumes constraints via `self` in `Propagate`, so even a
        // partial failure would otherwise leave the Instance in an inconsistent state.
        let mut working = self.clone();
        working.partial_evaluate_fallback_in_place(state, atol)?;
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
            let mut one_hot_replacements = BTreeMap::new();
            let mut one_hot_removals = BTreeMap::new();
            for (&id, oh) in self.one_hot_constraint_collection.active() {
                let (outcome, additional) = oh.clone().propagate(&expanded, atol)?;
                merge_state(&mut expanded, additional, atol, &mut changed)?;
                match outcome {
                    PropagateOutcome::Active(oh) => {
                        one_hot_replacements.insert(id, oh);
                    }
                    PropagateOutcome::Consumed(oh) => {
                        one_hot_removals.insert(id, (oh, propagation_reason.clone()));
                    }
                    PropagateOutcome::Transformed { new, .. } => match new {},
                }
            }
            self.one_hot_constraint_collection
                .replace_and_remove_active_rows(one_hot_replacements, one_hot_removals)?;

            // --- SOS1 constraints ---
            let mut sos1_replacements = BTreeMap::new();
            let mut sos1_removals = BTreeMap::new();
            for (&id, sos1) in self.sos1_constraint_collection.active() {
                let (outcome, additional) = sos1.clone().propagate(&expanded, atol)?;
                merge_state(&mut expanded, additional, atol, &mut changed)?;
                match outcome {
                    PropagateOutcome::Active(sos1) => {
                        sos1_replacements.insert(id, sos1);
                    }
                    PropagateOutcome::Consumed(sos1) => {
                        sos1_removals.insert(id, (sos1, propagation_reason.clone()));
                    }
                    PropagateOutcome::Transformed { new, .. } => match new {},
                }
            }
            self.sos1_constraint_collection
                .replace_and_remove_active_rows(sos1_replacements, sos1_removals)?;

            // --- Indicator constraints ---
            let indicator_context = self.indicator_constraint_collection.context();
            let mut indicator_replacements = BTreeMap::new();
            let mut indicator_removals = BTreeMap::new();
            let mut promoted_constraints = Vec::new();
            for (&id, ic) in self.indicator_constraint_collection.active() {
                let (outcome, additional) = ic.clone().propagate(&expanded, atol)?;
                merge_state(&mut expanded, additional, atol, &mut changed)?;
                match outcome {
                    PropagateOutcome::Active(ic) => {
                        indicator_replacements.insert(id, ic);
                    }
                    PropagateOutcome::Consumed(ic) => {
                        indicator_removals.insert(id, (ic, propagation_reason.clone()));
                    }
                    PropagateOutcome::Transformed {
                        original,
                        new: constraint,
                    } => {
                        // Indicator=1 → promote inner constraint to regular constraint.
                        // Carry over the indicator's context into the regular collection's
                        // store and record the promotion in provenance.
                        let mut new_context = indicator_context.collect_for(id);
                        new_context
                            .provenance
                            .push(crate::constraint::Provenance::IndicatorConstraint(id));
                        promoted_constraints.push((constraint, new_context));
                        indicator_removals.insert(id, (original, propagation_reason.clone()));
                    }
                }
            }
            self.indicator_constraint_collection
                .replace_and_remove_active_rows(indicator_replacements, indicator_removals)?;
            for (constraint, context) in promoted_constraints {
                let id = self.constraint_collection.unused_id();
                self.constraint_collection
                    .insert_active_with_context(id, constraint, context)?;
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
            (instance, state, (u, v)) in Instance::arbitrary_with(crate::InstanceParameters::regular_only())
                .prop_flat_map(|instance| {
                    let state = instance.arbitrary_state();
                    (Just(instance), state).prop_flat_map(|(instance, state)| {
                        let split = arbitrary_split_state(&state);
                        (Just(instance), Just(state), split)
                    })
                })
        ) {
            let s1 = instance.evaluate(&state, ATol::default()).unwrap();
            let mut borrowed = instance.clone();
            borrowed.partial_evaluate(&u, ATol::default()).unwrap();
            let consumed = instance
                .into_partial_evaluated(&u, ATol::default())
                .unwrap();
            prop_assert_eq!(&borrowed, &consumed);

            let s2 = consumed.evaluate(&v, ATol::default()).unwrap();
            prop_assert!(s1.state().abs_diff_eq(&s2.state(), ATol::default()));
        }
    }

    fn state_validation_instance() -> Instance {
        Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from((linear!(1) + linear!(2)).unwrap()))
            .decision_variables(BTreeMap::from([
                (VariableID::from(1), crate::DecisionVariable::continuous()),
                (VariableID::from(2), crate::DecisionVariable::continuous()),
            ]))
            .constraints(BTreeMap::new())
            .build()
            .unwrap()
    }

    #[test]
    fn test_populate_state_preserves_individual_state_shape_signals() {
        let instance = state_validation_instance();

        let missing = instance
            .populate_state(v1::State::default(), ATol::default())
            .unwrap_err();
        assert_eq!(
            missing
                .downcast_ref::<MissingStateEntries>()
                .map(|error| &error.ids),
            Some(&VariableIDSet::from([
                VariableID::from(1),
                VariableID::from(2),
            ]))
        );

        let unknown = instance
            .populate_state(
                v1::State::from(HashMap::from([(1, 0.0), (99, 0.0)])),
                ATol::default(),
            )
            .unwrap_err();
        assert_eq!(
            unknown
                .downcast_ref::<UnknownStateEntries>()
                .map(|error| &error.ids),
            Some(&VariableIDSet::from([VariableID::from(99)]))
        );
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
        assert!(matches!(
            err.downcast_ref::<DecisionVariableError>(),
            Some(DecisionVariableError::NonFiniteValue { id, value })
                if *id == VariableID::from(2) && value.is_nan()
        ));
        assert!(err.to_string().contains("must be finite"));

        let error = instance
            .populate_state(
                v1::State::from(HashMap::from([(1, 1.0), (2, 4.0)])),
                ATol::default(),
            )
            .unwrap_err();
        assert!(matches!(
            error.downcast_ref::<DecisionVariableError>(),
            Some(DecisionVariableError::SubstitutedValueOverwrite {
                id,
                previous_value,
                new_value,
                ..
            }) if *id == VariableID::from(2)
                && *previous_value == 3.0
                && *new_value == 4.0
        ));
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

    fn removed_only_instance(fixed_values: BTreeMap<VariableID, f64>) -> (Instance, ConstraintID) {
        let constraint_id = ConstraintID::from(1);
        let removed_reason = RemovedReason {
            reason: "test".to_string(),
            parameters: Default::default(),
        };
        let instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::from([
                (VariableID::from(1), crate::DecisionVariable::continuous()),
                (VariableID::from(2), crate::DecisionVariable::continuous()),
            ]))
            .fixed_decision_variable_values(fixed_values)
            .constraints(BTreeMap::new())
            .removed_constraints(BTreeMap::from([(
                constraint_id,
                (
                    Constraint::equal_to_zero(Function::from(linear!(1) + linear!(2))),
                    removed_reason,
                ),
            )]))
            .build()
            .unwrap();
        (instance, constraint_id)
    }

    #[test]
    fn test_partial_evaluate_removed_only_fast_path_does_not_revalidate_existing_fixed_values() {
        let var_id = VariableID::from(1);
        let existing_value = 1.0000005;
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::from([(
                var_id,
                crate::DecisionVariable::integer(),
            )]))
            .fixed_decision_variable_values(BTreeMap::from([(var_id, existing_value)]))
            .constraints(BTreeMap::new())
            .build()
            .unwrap();

        instance
            .partial_evaluate(&v1::State::default(), ATol::new(1e-9).unwrap())
            .unwrap();

        assert_eq!(
            instance.fixed_decision_variable_value(var_id),
            Some(existing_value)
        );
    }

    #[test]
    fn test_partial_evaluate_removed_only_fast_path_preserves_restore_semantics() {
        let (mut instance, constraint_id) = removed_only_instance(BTreeMap::new());
        let removed_before = instance.removed_constraints().clone();

        let state = v1::State::from(HashMap::from([(1, 3.0), (2, 4.0)]));
        instance.partial_evaluate(&state, ATol::default()).unwrap();

        assert!(instance.constraints().is_empty());
        assert_eq!(instance.removed_constraints(), &removed_before);
        assert_eq!(
            instance.fixed_decision_variable_values(),
            &BTreeMap::from([(VariableID::from(1), 3.0), (VariableID::from(2), 4.0)])
        );

        instance.restore_constraint(constraint_id).unwrap();
        let restored = instance.constraints().get(&constraint_id).unwrap();
        assert!(restored.required_ids().is_empty());
    }

    #[test]
    fn test_into_partial_evaluated_removed_only_fast_path_preserves_restore_semantics() {
        let (instance, constraint_id) = removed_only_instance(BTreeMap::new());
        let removed_before = instance.removed_constraints().clone();

        let state = v1::State::from(HashMap::from([(1, 3.0), (2, 4.0)]));
        let mut instance = instance
            .into_partial_evaluated(&state, ATol::default())
            .unwrap();

        assert!(instance.constraints().is_empty());
        assert_eq!(instance.removed_constraints(), &removed_before);
        assert_eq!(
            instance.fixed_decision_variable_values(),
            &BTreeMap::from([(VariableID::from(1), 3.0), (VariableID::from(2), 4.0)])
        );

        instance.restore_constraint(constraint_id).unwrap();
        let restored = instance.constraints().get(&constraint_id).unwrap();
        assert!(restored.required_ids().is_empty());
    }

    #[test]
    fn test_partial_evaluate_removed_only_fast_path_rejects_conflict_atomically() {
        let (mut instance, _constraint_id) =
            removed_only_instance(BTreeMap::from([(VariableID::from(2), 0.0)]));
        let fixed_before = instance.fixed_decision_variable_values().clone();
        let removed_before = instance.removed_constraints().clone();

        let state = v1::State::from(HashMap::from([(1, 1.0), (2, 2.0)]));
        let err = instance
            .partial_evaluate(&state, ATol::default())
            .unwrap_err();

        assert!(matches!(
            err.downcast_ref::<DecisionVariableError>(),
            Some(DecisionVariableError::SubstitutedValueOverwrite {
                id,
                previous_value,
                new_value,
                ..
            }) if *id == VariableID::from(2)
                && *previous_value == 0.0
                && *new_value == 2.0
        ));
        assert_eq!(instance.fixed_decision_variable_values(), &fixed_before);
        assert_eq!(instance.removed_constraints(), &removed_before);
    }

    #[test]
    fn test_partial_evaluate_fixed_values_only_fast_path_allows_removed_special_rows() {
        use crate::{DecisionVariable, OneHotConstraint, OneHotConstraintID};

        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::from([
                (VariableID::from(1), DecisionVariable::binary()),
                (VariableID::from(2), DecisionVariable::binary()),
                (VariableID::from(3), DecisionVariable::binary()),
                (VariableID::from(4), DecisionVariable::continuous()),
            ]))
            .constraints(BTreeMap::new())
            .build()
            .unwrap();
        instance
            .one_hot_constraint_collection
            .insert_active_with_context(
                OneHotConstraintID::from(1),
                OneHotConstraint::new([1, 2, 3].into_iter().map(VariableID::from).collect())
                    .unwrap(),
                crate::ConstraintContext::default(),
            )
            .unwrap();
        instance
            .partial_evaluate(&v1::State::from(HashMap::from([(2, 1.0)])), ATol::default())
            .unwrap();

        let removed_before = instance.removed_one_hot_constraints().clone();
        let state = v1::State::from(HashMap::from([(4, 2.0)]));
        assert!(matches!(
            PartialEvaluatePlan::prepare(&instance, &state, ATol::default()).unwrap(),
            Some(PartialEvaluatePlan::FixedValuesOnly { .. })
        ));

        instance.partial_evaluate(&state, ATol::default()).unwrap();

        assert_eq!(
            instance.fixed_decision_variable_value(VariableID::from(4)),
            Some(2.0)
        );
        assert_eq!(instance.removed_one_hot_constraints(), &removed_before);
    }

    fn regular_plan_instance() -> Instance {
        let named_function_id = crate::NamedFunctionID::from(1);
        let constraint_id = ConstraintID::from(1);
        let remaining_constraint_id = ConstraintID::from(2);
        let removed_constraint_id = ConstraintID::from(10);
        let removed_reason = RemovedReason {
            reason: "test".to_string(),
            parameters: Default::default(),
        };
        Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from((linear!(1) + linear!(3)).unwrap()))
            .decision_variables(BTreeMap::from([
                (VariableID::from(1), crate::DecisionVariable::continuous()),
                (VariableID::from(2), crate::DecisionVariable::continuous()),
                (VariableID::from(3), crate::DecisionVariable::continuous()),
            ]))
            .fixed_decision_variable_values(BTreeMap::from([(VariableID::from(2), 3.0)]))
            .constraints(BTreeMap::from([
                (
                    constraint_id,
                    Constraint::equal_to_zero(Function::from((linear!(1) + linear!(3)).unwrap())),
                ),
                (
                    remaining_constraint_id,
                    Constraint::equal_to_zero(Function::from(linear!(3))),
                ),
            ]))
            .removed_constraints(BTreeMap::from([(
                removed_constraint_id,
                (
                    Constraint::equal_to_zero(Function::from(linear!(2))),
                    removed_reason,
                ),
            )]))
            .named_functions(BTreeMap::from([(
                named_function_id,
                crate::NamedFunction {
                    function: Function::from(
                        ((linear!(1) + linear!(2)).unwrap() + linear!(3)).unwrap(),
                    ),
                },
            )]))
            .build()
            .unwrap()
    }

    #[test]
    fn test_partial_evaluate_regular_plan_matches_consuming_path() {
        let instance = regular_plan_instance();
        let state = v1::State::from(HashMap::from([(1, 2.0)]));

        assert!(matches!(
            PartialEvaluatePlan::prepare(&instance, &state, ATol::default()).unwrap(),
            Some(PartialEvaluatePlan::RegularReplacement { .. })
        ));

        let mut borrowed = instance.clone();
        borrowed.partial_evaluate(&state, ATol::default()).unwrap();
        let consumed = instance
            .clone()
            .into_partial_evaluated(&state, ATol::default())
            .unwrap();

        assert_eq!(borrowed, consumed);
        assert_eq!(
            borrowed.fixed_decision_variable_values(),
            &BTreeMap::from([(VariableID::from(1), 2.0), (VariableID::from(2), 3.0)])
        );
        assert_eq!(
            borrowed.objective().required_ids(),
            VariableIDSet::from([VariableID::from(3)])
        );
        assert_eq!(
            borrowed
                .constraints()
                .get(&ConstraintID::from(1))
                .unwrap()
                .required_ids(),
            VariableIDSet::from([VariableID::from(3)])
        );

        let original_solution = instance
            .evaluate(
                &v1::State::from(HashMap::from([(1, 2.0), (3, 5.0)])),
                ATol::default(),
            )
            .unwrap();
        let rewritten_solution = borrowed
            .evaluate(&v1::State::from(HashMap::from([(3, 5.0)])), ATol::default())
            .unwrap();
        assert_eq!(
            original_solution.objective(),
            rewritten_solution.objective()
        );
        assert_eq!(
            original_solution
                .evaluated_named_functions()
                .get(&crate::NamedFunctionID::from(1))
                .unwrap()
                .evaluated_value(),
            rewritten_solution
                .evaluated_named_functions()
                .get(&crate::NamedFunctionID::from(1))
                .unwrap()
                .evaluated_value()
        );
    }

    #[test]
    fn test_partial_evaluate_regular_plan_error_leaves_original_unchanged() {
        let removed_reason = RemovedReason {
            reason: "test".to_string(),
            parameters: Default::default(),
        };
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from((coeff!(f64::MAX) * linear!(1)).unwrap()))
            .decision_variables(BTreeMap::from([(
                VariableID::from(1),
                crate::DecisionVariable::continuous(),
            )]))
            .constraints(BTreeMap::new())
            .removed_constraints(BTreeMap::from([(
                ConstraintID::from(1),
                (
                    Constraint::equal_to_zero(Function::from(linear!(1))),
                    removed_reason,
                ),
            )]))
            .build()
            .unwrap();
        assert!(matches!(
            PartialEvaluatePlan::prepare(
                &instance,
                &v1::State::from(HashMap::from([(1, 1.0)])),
                ATol::default(),
            )
            .unwrap(),
            Some(PartialEvaluatePlan::RegularReplacement { .. })
        ));
        let before = instance.clone();

        let err = instance
            .partial_evaluate(
                &v1::State::from(HashMap::from([(1, f64::MAX)])),
                ATol::default(),
            )
            .unwrap_err();

        assert!(
            err.to_string().contains("finite"),
            "unexpected error: {err}"
        );
        assert_eq!(instance, before);
    }

    #[test]
    fn test_partial_evaluate_active_special_constraint_stays_on_fallback_path() {
        use crate::{DecisionVariable, OneHotConstraint, OneHotConstraintID};

        let removed_reason = RemovedReason {
            reason: "test".to_string(),
            parameters: Default::default(),
        };
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(
                ((linear!(1) + linear!(2)).unwrap() + linear!(3)).unwrap(),
            ))
            .decision_variables(BTreeMap::from([
                (VariableID::from(1), DecisionVariable::binary()),
                (VariableID::from(2), DecisionVariable::binary()),
                (VariableID::from(3), DecisionVariable::binary()),
            ]))
            .constraints(BTreeMap::new())
            .removed_constraints(BTreeMap::from([(
                ConstraintID::from(10),
                (
                    Constraint::equal_to_zero(Function::from(linear!(1))),
                    removed_reason,
                ),
            )]))
            .build()
            .unwrap();
        let state = v1::State::from(HashMap::from([(2, 1.0)]));
        assert!(matches!(
            PartialEvaluatePlan::prepare(&instance, &state, ATol::default()).unwrap(),
            Some(PartialEvaluatePlan::RegularReplacement { .. })
        ));
        instance
            .one_hot_constraint_collection
            .insert_active_with_context(
                OneHotConstraintID::from(1),
                OneHotConstraint::new([1, 2, 3].into_iter().map(VariableID::from).collect())
                    .unwrap(),
                crate::ConstraintContext::default(),
            )
            .unwrap();

        assert!(
            PartialEvaluatePlan::prepare(&instance, &state, ATol::default())
                .unwrap()
                .is_none()
        );

        let mut borrowed = instance.clone();
        borrowed.partial_evaluate(&state, ATol::default()).unwrap();
        let consumed = instance
            .into_partial_evaluated(&state, ATol::default())
            .unwrap();

        assert_eq!(borrowed, consumed);
        assert!(borrowed.one_hot_constraint_collection.active().is_empty());
        assert_eq!(borrowed.one_hot_constraint_collection.removed().len(), 1);
        assert_eq!(
            borrowed.fixed_decision_variable_values(),
            &BTreeMap::from([
                (VariableID::from(1), 0.0),
                (VariableID::from(2), 1.0),
                (VariableID::from(3), 0.0),
            ])
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

    fn dependent_instance_y_eq_2x() -> Instance {
        let decision_variables = BTreeMap::from([
            (VariableID::from(1), crate::DecisionVariable::continuous()),
            (VariableID::from(10), crate::DecisionVariable::continuous()),
        ]);
        let removed_reason = RemovedReason {
            reason: "test".to_string(),
            parameters: Default::default(),
        };
        Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(1)))
            .decision_variables(decision_variables)
            .constraints(BTreeMap::new())
            .removed_constraints(BTreeMap::from([(
                ConstraintID::from(1),
                (
                    Constraint::equal_to_zero(Function::from(linear!(1))),
                    removed_reason,
                ),
            )]))
            .decision_variable_dependency(crate::assign! {
                10 <- coeff!(2.0) * linear!(1)
            })
            .build()
            .unwrap()
    }

    #[test]
    fn test_partial_evaluate_normalizes_constant_dependency_from_input() {
        let mut instance = dependent_instance_y_eq_2x();

        let state = v1::State::from(HashMap::from([(1, 2.0)]));
        assert!(
            PartialEvaluatePlan::prepare(&instance, &state, ATol::default())
                .unwrap()
                .is_none()
        );

        instance.partial_evaluate(&state, ATol::default()).unwrap();

        assert_eq!(
            instance.fixed_decision_variable_values(),
            &BTreeMap::from([(VariableID::from(1), 2.0), (VariableID::from(10), 4.0)])
        );
        assert!(instance.decision_variable_dependency.is_empty());
        assert_eq!(
            instance
                .populate_state(v1::State::default(), ATol::default())
                .unwrap()
                .entries,
            HashMap::from([(1, 2.0), (10, 4.0)])
        );
    }

    #[test]
    fn test_partial_evaluate_dependency_overflow_is_unclassified_and_atomic() {
        let mut instance = dependent_instance_y_eq_2x();
        instance.decision_variable_dependency = crate::assign! {
            10 <- coeff!(f64::MAX) * linear!(1)
        };
        let before = instance.clone();

        let error = instance
            .partial_evaluate(
                &v1::State::from(HashMap::from([(1, f64::MAX)])),
                ATol::default(),
            )
            .unwrap_err();

        assert!(!error.is::<crate::CoefficientError>());
        assert!(error
            .to_string()
            .contains("failed to normalize dependent variable"));
        assert_eq!(instance, before);
    }

    #[test]
    fn test_partial_evaluate_accepts_consistent_dependent_assertion() {
        let mut instance = dependent_instance_y_eq_2x();

        let state = v1::State::from(HashMap::from([(1, 2.0), (10, 4.0)]));
        instance.partial_evaluate(&state, ATol::default()).unwrap();

        assert_eq!(
            instance.fixed_decision_variable_values(),
            &BTreeMap::from([(VariableID::from(1), 2.0), (VariableID::from(10), 4.0)])
        );
        assert!(instance.decision_variable_dependency.is_empty());
    }

    #[test]
    fn test_partial_evaluate_rejects_inconsistent_dependent_assertion_atomically() {
        let mut instance = dependent_instance_y_eq_2x();

        let state = v1::State::from(HashMap::from([(1, 2.0), (10, 5.0)]));
        let err = instance
            .partial_evaluate(&state, ATol::default())
            .unwrap_err();

        assert!(matches!(
            err.downcast_ref::<InconsistentDependentValue>(),
            Some(InconsistentDependentValue {
                id,
                state_value,
                dependency_value,
            }) if *id == VariableID::from(10)
                && *state_value == 5.0
                && *dependency_value == 4.0
        ));
        assert!(
            err.to_string()
                .contains("state value for dependent variable VariableID(10) is inconsistent"),
            "unexpected error: {err}"
        );
        assert!(instance.fixed_decision_variable_values().is_empty());
        assert!(instance
            .decision_variable_dependency
            .get(&VariableID::from(10))
            .is_some());
    }

    #[test]
    fn test_populate_state_preserves_inconsistent_dependent_value_payload() {
        let instance = dependent_instance_y_eq_2x();
        let state = v1::State::from(HashMap::from([(1, 2.0), (10, 5.0)]));

        let err = instance.populate_state(state, ATol::default()).unwrap_err();

        assert!(matches!(
            err.downcast_ref::<InconsistentDependentValue>(),
            Some(InconsistentDependentValue {
                id,
                state_value,
                dependency_value,
            }) if *id == VariableID::from(10)
                && *state_value == 5.0
                && *dependency_value == 4.0
        ));
    }

    #[test]
    fn test_partial_evaluate_rejects_unverifiable_dependent_assertion_atomically() {
        let mut instance = dependent_instance_y_eq_2x();

        let state = v1::State::from(HashMap::from([(10, 4.0)]));
        let err = instance
            .partial_evaluate(&state, ATol::default())
            .unwrap_err();

        assert_eq!(
            err.downcast_ref::<UnverifiableDependentAssertion>()
                .map(|error| (error.id, &error.required_ids)),
            Some((
                VariableID::from(10),
                &VariableIDSet::from([VariableID::from(1)]),
            ))
        );
        assert!(
            err.to_string()
                .contains("Dependent variable (ID=10) cannot be asserted"),
            "unexpected error: {err}"
        );
        assert!(instance.fixed_decision_variable_values().is_empty());
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
    fn test_partial_evaluate_normalizes_dependency_chain_in_order() {
        let decision_variables = BTreeMap::from([
            (VariableID::from(1), crate::DecisionVariable::continuous()),
            (VariableID::from(10), crate::DecisionVariable::continuous()),
            (VariableID::from(11), crate::DecisionVariable::continuous()),
        ]);
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(1)))
            .decision_variables(decision_variables)
            .constraints(BTreeMap::new())
            .decision_variable_dependency(crate::assign! {
                10 <- coeff!(2.0) * linear!(1),
                11 <- linear!(10) + coeff!(1.0)
            })
            .build()
            .unwrap();

        let state = v1::State::from(HashMap::from([(1, 2.0)]));
        instance.partial_evaluate(&state, ATol::default()).unwrap();

        assert_eq!(
            instance.fixed_decision_variable_values(),
            &BTreeMap::from([
                (VariableID::from(1), 2.0),
                (VariableID::from(10), 4.0),
                (VariableID::from(11), 5.0),
            ])
        );
        assert!(instance.decision_variable_dependency.is_empty());
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
        assert!(!err.is::<MissingStateEntries>());
        assert!(!err.is::<UnknownStateEntries>());
        assert!(!err.is::<InconsistentDependentValue>());
        assert!(!err.is::<UnverifiableDependentAssertion>());
        assert!(!err.is::<DecisionVariableError>());
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
    fn test_partial_evaluate_special_constraints_validate_state_before_propagation() {
        use crate::{DecisionVariable, OneHotConstraint, OneHotConstraintID};

        let mut instance = Instance::new(
            Sense::Minimize,
            Function::Zero,
            BTreeMap::from([
                (VariableID::from(1), DecisionVariable::binary()),
                (VariableID::from(2), DecisionVariable::binary()),
            ]),
            BTreeMap::new(),
        )
        .unwrap();
        instance
            .one_hot_constraint_collection
            .insert_active_with_context(
                OneHotConstraintID::from(1),
                OneHotConstraint::new(
                    [VariableID::from(1), VariableID::from(2)]
                        .into_iter()
                        .collect(),
                )
                .unwrap(),
                crate::ConstraintContext::default(),
            )
            .unwrap();

        let unknown = instance
            .partial_evaluate(
                &v1::State::from(HashMap::from([(99, 0.0), (100, 0.0)])),
                ATol::default(),
            )
            .unwrap_err();
        assert_eq!(
            unknown
                .downcast_ref::<UnknownStateEntries>()
                .map(|error| &error.ids),
            Some(&VariableIDSet::from([
                VariableID::from(99),
                VariableID::from(100),
            ]))
        );

        let non_finite = instance
            .partial_evaluate(
                &v1::State::from(HashMap::from([(1, f64::NAN)])),
                ATol::default(),
            )
            .unwrap_err();
        assert!(matches!(
            non_finite.downcast_ref::<DecisionVariableError>(),
            Some(DecisionVariableError::NonFiniteValue { id, value })
                if *id == VariableID::from(1) && value.is_nan()
        ));

        let inconsistent = instance
            .partial_evaluate(&v1::State::from(HashMap::from([(1, 0.5)])), ATol::default())
            .unwrap_err();
        assert!(matches!(
            inconsistent.downcast_ref::<DecisionVariableError>(),
            Some(DecisionVariableError::SubstitutedValueInconsistent { id, .. })
                if *id == VariableID::from(1)
        ));

        assert_eq!(instance.one_hot_constraint_collection.active().len(), 1);
        assert!(instance.fixed_decision_variable_values().is_empty());
    }

    #[test]
    fn test_partial_evaluate_derived_value_failure_is_unclassified_and_atomic() {
        use crate::{DecisionVariable, Sos1Constraint, Sos1ConstraintID};

        let bounded = DecisionVariable::new(
            Kind::Continuous,
            Bound::new(1.0, 2.0).unwrap(),
            ATol::default(),
        )
        .unwrap();
        let mut instance = Instance::new(
            Sense::Minimize,
            Function::Zero,
            BTreeMap::from([
                (VariableID::from(1), bounded),
                (VariableID::from(2), DecisionVariable::continuous()),
            ]),
            BTreeMap::new(),
        )
        .unwrap();
        instance
            .sos1_constraint_collection
            .insert_active_with_context(
                Sos1ConstraintID::from(1),
                Sos1Constraint::new(
                    [VariableID::from(1), VariableID::from(2)]
                        .into_iter()
                        .collect(),
                )
                .unwrap(),
                crate::ConstraintContext::default(),
            )
            .unwrap();
        let before = instance.clone();

        let error = instance
            .partial_evaluate(&v1::State::from(HashMap::from([(2, 1.0)])), ATol::default())
            .unwrap_err();

        assert!(error.downcast_ref::<DecisionVariableError>().is_none());
        assert!(error
            .to_string()
            .contains("special-constraint propagation produced an invalid value"));
        assert_eq!(instance, before);
    }

    #[test]
    fn test_partial_evaluate_rolls_back_changes_before_later_propagation_failure() {
        use crate::{
            DecisionVariable, OneHotConstraint, OneHotConstraintID, Sos1Constraint,
            Sos1ConstraintID,
        };

        let mut instance = Instance::new(
            Sense::Minimize,
            Function::Zero,
            BTreeMap::from([
                (VariableID::from(1), DecisionVariable::binary()),
                (VariableID::from(2), DecisionVariable::binary()),
                (VariableID::from(3), DecisionVariable::continuous()),
            ]),
            BTreeMap::new(),
        )
        .unwrap();
        instance
            .one_hot_constraint_collection
            .insert_active_with_context(
                OneHotConstraintID::from(1),
                OneHotConstraint::new(
                    [VariableID::from(1), VariableID::from(2)]
                        .into_iter()
                        .collect(),
                )
                .unwrap(),
                crate::ConstraintContext::default(),
            )
            .unwrap();
        instance
            .sos1_constraint_collection
            .insert_active_with_context(
                Sos1ConstraintID::from(1),
                Sos1Constraint::new(
                    [VariableID::from(2), VariableID::from(3)]
                        .into_iter()
                        .collect(),
                )
                .unwrap(),
                crate::ConstraintContext::default(),
            )
            .unwrap();
        let before = instance.clone();

        instance
            .partial_evaluate(
                &v1::State::from(HashMap::from([(1, 0.0), (3, 1.0)])),
                ATol::default(),
            )
            .unwrap_err();

        assert_eq!(instance, before);
    }

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
            .insert_active_with_context(
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
            .insert_active_with_context(
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
            .insert_active_with_context(
                OneHotConstraintID::from(1),
                oh,
                crate::ConstraintContext::default(),
            )
            .unwrap();

        let sos1 = Sos1Constraint::new([2, 3].into_iter().map(VariableID::from).collect()).unwrap();
        instance
            .sos1_constraint_collection
            .insert_active_with_context(
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
