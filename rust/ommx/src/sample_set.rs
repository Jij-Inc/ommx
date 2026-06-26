mod extract;
mod parse;
mod serialize;

use crate::{
    constraint_type::{EvaluatedCollection, SampledCollection, SampledConstraintBehavior},
    indicator_constraint::IndicatorConstraint,
    Constraint, ConstraintID, EvaluatedConstraint, EvaluatedDecisionVariable,
    EvaluatedNamedFunction, NamedFunctionID, SampleID, SampleIDSet, Sampled, SampledConstraint,
    SampledDecisionVariable, SampledNamedFunction, Sense, Solution, VariableID,
};
use getset::Getters;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Error occurred during SampleSet validation
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum SampleSetError {
    #[error("Inconsistent feasibility for sample {sample_id}: provided={provided_feasible}, computed={computed_feasible}")]
    InconsistentFeasibility {
        sample_id: u64,
        provided_feasible: bool,
        computed_feasible: bool,
    },

    #[error("Inconsistent feasibility (relaxed) for sample {sample_id}: provided={provided_feasible_relaxed}, computed={computed_feasible_relaxed}")]
    InconsistentFeasibilityRelaxed {
        sample_id: u64,
        provided_feasible_relaxed: bool,
        computed_feasible_relaxed: bool,
    },

    #[error("Inconsistent sample IDs: expected {expected:?}, found {found:?}")]
    InconsistentSampleIDs {
        expected: SampleIDSet,
        found: SampleIDSet,
    },

    #[error("Duplicated variable ID is found in definition: {id:?}")]
    DuplicatedVariableID { id: VariableID },

    #[error("Duplicate subscripts for {name}: {subscripts:?}")]
    DuplicateSubscripts { name: String, subscripts: Vec<i64> },

    #[error("No decision variables with name '{name}' found")]
    UnknownVariableName { name: String },

    #[error("No constraint with name '{name}' found")]
    UnknownConstraintName { name: String },

    #[deprecated(
        note = "Parameters are now ignored in extract_decision_variables and extract_all_decision_variables"
    )]
    #[error("Decision variable with parameters is not supported")]
    ParameterizedVariable,

    #[error("Constraint with parameters is not supported")]
    ParameterizedConstraint,

    #[error("Unknown sample ID: {id:?}")]
    UnknownSampleID { id: SampleID },

    #[error("No feasible solution found")]
    NoFeasibleSolution,

    #[error("No feasible solution found in relaxed problem")]
    NoFeasibleSolutionRelaxed,

    #[error("No named function with name '{name}' found")]
    UnknownNamedFunctionName { name: String },

    #[deprecated(
        note = "Parameters are now allowed in extract methods; only subscripts are used as keys"
    )]
    #[error("Named function with parameters is not supported")]
    ParameterizedNamedFunction,

    #[error("Required field is missing: {field}")]
    MissingRequiredField { field: &'static str },

    #[error("{message}")]
    InvalidSidecar { message: String },
}

/// Multiple sample solution results with deduplication
///
/// Invariants
/// -----------
/// - [`Self::decision_variables`] is keyed by the table-owned
///   [`VariableID`]; sampled decision-variable rows do not carry IDs.
/// - [`Self::named_functions`] is keyed by the table-owned
///   [`NamedFunctionID`]; sampled named-function rows do not carry IDs.
/// - All [`Self::decision_variables`], [`Self::objectives`], sampled constraint
///   collections, and [`Self::named_functions`] have the same sample ID set.
/// - [`Self::feasible`] and [`Self::feasible_relaxed`] are computed from all
///   sampled constraint collections:
///   - `feasible`: true if all constraints are satisfied for that sample.
///   - `feasible_relaxed`: true if all non-removed constraints are satisfied.
#[derive(Debug, Clone, Getters)]
pub struct SampleSet {
    #[getset(get = "pub")]
    decision_variables: BTreeMap<VariableID, SampledDecisionVariable>,
    /// Per-variable modeling labels (sibling of [`Self::decision_variables`]).
    #[getset(get = "pub")]
    variable_labels: crate::decision_variable::VariableLabelStore,
    #[getset(get = "pub")]
    objectives: Sampled<f64>,
    #[getset(get = "pub")]
    constraints: SampledCollection<Constraint>,
    #[getset(get = "pub")]
    indicator_constraints: SampledCollection<IndicatorConstraint>,
    #[getset(get = "pub")]
    one_hot_constraints: SampledCollection<crate::OneHotConstraint>,
    #[getset(get = "pub")]
    sos1_constraints: SampledCollection<crate::Sos1Constraint>,
    #[getset(get = "pub")]
    named_functions: BTreeMap<NamedFunctionID, SampledNamedFunction>,
    /// Per-named-function modeling labels (sibling of [`Self::named_functions`]).
    #[getset(get = "pub")]
    named_function_labels: crate::named_function::NamedFunctionLabelStore,
    #[getset(get = "pub")]
    sense: Sense,
    #[getset(get = "pub")]
    feasible: BTreeMap<SampleID, bool>,
    #[getset(get = "pub")]
    feasible_relaxed: BTreeMap<SampleID, bool>,
    /// OMMX-defined provenance metadata.
    pub metadata: Option<crate::v1::ProcessMetadata>,
    /// User-defined or third-party extension annotations.
    pub annotations: HashMap<String, String>,
}

impl SampleSet {
    /// Create a new SampleSet
    ///
    /// # Deprecated
    /// This constructor does not support named functions.
    /// Use [`SampleSetBuilder::build`] for full functionality.
    #[deprecated(
        since = "2.5.0",
        note = "Use SampleSet::builder().build() for construction with named_functions support"
    )]
    pub fn new(
        decision_variables: BTreeMap<VariableID, SampledDecisionVariable>,
        objectives: Sampled<f64>,
        constraints: BTreeMap<ConstraintID, SampledConstraint>,
        sense: Sense,
    ) -> Result<Self, SampleSetError> {
        Self::builder()
            .decision_variables(decision_variables)
            .objectives(objectives)
            .constraints(constraints)
            .sense(sense)
            .build()
    }

    /// Get sample IDs available in this sample set
    pub fn sample_ids(&self) -> SampleIDSet {
        self.objectives.ids()
    }

    pub fn feasible_ids(&self) -> SampleIDSet {
        self.feasible
            .iter()
            .filter_map(|(id, &is_feasible)| if is_feasible { Some(*id) } else { None })
            .collect()
    }

    pub fn feasible_relaxed_ids(&self) -> SampleIDSet {
        self.feasible_relaxed
            .iter()
            .filter_map(|(id, &is_feasible)| if is_feasible { Some(*id) } else { None })
            .collect()
    }

    pub fn feasible_unrelaxed_ids(&self) -> SampleIDSet {
        self.feasible_ids()
    }

    /// Check if a specific sample is feasible.
    ///
    /// Returns [`None`] if `sample_id` is not in this sample set.
    pub fn is_sample_feasible(&self, sample_id: SampleID) -> Option<bool> {
        self.feasible.get(&sample_id).copied()
    }

    /// Check if a specific sample is feasible in the relaxed problem.
    ///
    /// Returns [`None`] if `sample_id` is not in this sample set.
    pub fn is_sample_feasible_relaxed(&self, sample_id: SampleID) -> Option<bool> {
        self.feasible_relaxed.get(&sample_id).copied()
    }

    /// Get a specific solution by sample ID.
    ///
    /// Returns [`None`] if `sample_id` is not in this sample set.
    pub fn get(&self, sample_id: crate::SampleID) -> Option<Solution> {
        // Get objective value
        let objective = *self.objectives.get(sample_id)?;

        // Get decision variables with substituted values - convert to EvaluatedDecisionVariable
        let mut decision_variables: BTreeMap<VariableID, EvaluatedDecisionVariable> =
            BTreeMap::default();
        for (variable_id, sampled_dv) in &self.decision_variables {
            let evaluated_dv = sampled_dv.get(*variable_id, sample_id)?;
            decision_variables.insert(*variable_id, evaluated_dv);
        }

        // Get evaluated constraints
        let mut evaluated_constraints: BTreeMap<ConstraintID, EvaluatedConstraint> =
            BTreeMap::default();
        for (constraint_id, constraint) in self.constraints.iter() {
            let evaluated_constraint = constraint.get(sample_id)?;
            evaluated_constraints.insert(*constraint_id, evaluated_constraint);
        }

        // Get evaluated indicator constraints
        let mut evaluated_indicator_constraints = BTreeMap::default();
        for (constraint_id, constraint) in self.indicator_constraints.iter() {
            use crate::constraint_type::SampledConstraintBehavior;
            let evaluated = constraint.get(sample_id)?;
            evaluated_indicator_constraints.insert(*constraint_id, evaluated);
        }

        // Get evaluated one-hot constraints
        let mut evaluated_one_hot_constraints = BTreeMap::default();
        for (constraint_id, constraint) in self.one_hot_constraints.iter() {
            use crate::constraint_type::SampledConstraintBehavior;
            let evaluated = constraint.get(sample_id)?;
            evaluated_one_hot_constraints.insert(*constraint_id, evaluated);
        }

        // Get evaluated SOS1 constraints
        let mut evaluated_sos1_constraints = BTreeMap::default();
        for (constraint_id, constraint) in self.sos1_constraints.iter() {
            use crate::constraint_type::SampledConstraintBehavior;
            let evaluated = constraint.get(sample_id)?;
            evaluated_sos1_constraints.insert(*constraint_id, evaluated);
        }

        // Get evaluated named functions
        let mut evaluated_named_functions: BTreeMap<NamedFunctionID, EvaluatedNamedFunction> =
            BTreeMap::default();
        for (named_function_id, named_function) in &self.named_functions {
            let evaluated_named_function = named_function.get(sample_id)?;
            evaluated_named_functions.insert(*named_function_id, evaluated_named_function);
        }

        let sense = *self.sense();

        // SAFETY: SampleSet invariants guarantee Solution invariants.
        // Constraint label/provenance stores ride along from the source
        // SampledCollection so per-sample Solutions retain modeling labels and
        // transformation lineage attached at the SampleSet level.
        Some(unsafe {
            Solution::builder()
                .evaluated_constraints_collection(
                    EvaluatedCollection::with_context(
                        evaluated_constraints,
                        self.constraints.removed_reasons().clone(),
                        self.constraints.context().clone(),
                    )
                    .expect("SampleSet sidecars must reference constraints present in this sample"),
                )
                .evaluated_indicator_constraints_collection(
                    EvaluatedCollection::with_context(
                        evaluated_indicator_constraints,
                        self.indicator_constraints.removed_reasons().clone(),
                        self.indicator_constraints.context().clone(),
                    )
                    .expect("SampleSet sidecars must reference constraints present in this sample"),
                )
                .evaluated_one_hot_constraints_collection(
                    EvaluatedCollection::with_context(
                        evaluated_one_hot_constraints,
                        self.one_hot_constraints.removed_reasons().clone(),
                        self.one_hot_constraints.context().clone(),
                    )
                    .expect("SampleSet sidecars must reference constraints present in this sample"),
                )
                .evaluated_sos1_constraints_collection(
                    EvaluatedCollection::with_context(
                        evaluated_sos1_constraints,
                        self.sos1_constraints.removed_reasons().clone(),
                        self.sos1_constraints.context().clone(),
                    )
                    .expect("SampleSet sidecars must reference constraints present in this sample"),
                )
                .objective(objective)
                .evaluated_named_functions(evaluated_named_functions)
                .decision_variables(decision_variables)
                .variable_labels(self.variable_labels.clone())
                .named_function_labels(self.named_function_labels.clone())
                .sense(sense)
                .build_unchecked()
                .expect("SampleSet invariants guarantee Solution invariants")
        })
    }

    pub fn best_feasible_id(&self) -> Result<SampleID, SampleSetError> {
        let mut feasible_objectives: Vec<(SampleID, f64)> = self
            .feasible
            .iter()
            .filter_map(|(k, v)| if *v { Some(k) } else { None })
            .map(|id| (*id, *self.objectives.get(*id).unwrap())) // safe unwrap since the IDs are consistent
            .collect();
        if feasible_objectives.is_empty() {
            return Err(SampleSetError::NoFeasibleSolution);
        }
        feasible_objectives.sort_by(|a, b| a.1.total_cmp(&b.1));
        match self.sense {
            // safe unwrap since we checked for non-empty feasible_objectives
            Sense::Minimize => Ok(feasible_objectives.first().unwrap().0),
            Sense::Maximize => Ok(feasible_objectives.last().unwrap().0),
        }
    }

    pub fn best_feasible_relaxed_id(&self) -> Result<SampleID, SampleSetError> {
        let mut feasible_objectives: Vec<(SampleID, f64)> = self
            .feasible_relaxed
            .iter()
            .filter_map(|(k, v)| if *v { Some(k) } else { None })
            .map(|id| (*id, *self.objectives.get(*id).unwrap())) // safe unwrap since the IDs are consistent
            .collect();
        if feasible_objectives.is_empty() {
            return Err(SampleSetError::NoFeasibleSolutionRelaxed);
        }
        feasible_objectives.sort_by(|a, b| a.1.total_cmp(&b.1));
        match self.sense {
            // safe unwrap since we checked for non-empty feasible_objectives
            Sense::Minimize => Ok(feasible_objectives.first().unwrap().0),
            Sense::Maximize => Ok(feasible_objectives.last().unwrap().0),
        }
    }

    /// Get the best feasible solution
    pub fn best_feasible(&self) -> Result<Solution, SampleSetError> {
        let id = self.best_feasible_id()?;
        self.get(id).ok_or(SampleSetError::UnknownSampleID { id })
    }

    pub fn best_feasible_relaxed(&self) -> Result<Solution, SampleSetError> {
        let id = self.best_feasible_relaxed_id()?;
        self.get(id).ok_or(SampleSetError::UnknownSampleID { id })
    }

    /// Creates a new [`SampleSetBuilder`].
    pub fn builder() -> SampleSetBuilder {
        SampleSetBuilder::new()
    }
}

/// Builder for creating [`SampleSet`] with validation.
///
/// # Example
/// ```
/// use ommx::{SampleSet, Sampled, Sense};
/// use std::collections::BTreeMap;
///
/// let sample_set = SampleSet::builder()
///     .decision_variables(BTreeMap::new())
///     .objectives(Sampled::default())
///     .constraints(BTreeMap::new())
///     .sense(Sense::Minimize)
///     .build()
///     .unwrap();
/// ```
#[derive(Debug, Clone, Default)]
pub struct SampleSetBuilder {
    decision_variables: Option<BTreeMap<VariableID, SampledDecisionVariable>>,
    variable_labels: crate::decision_variable::VariableLabelStore,
    objectives: Option<Sampled<f64>>,
    constraints: Option<SampledCollection<Constraint>>,
    indicator_constraints: SampledCollection<IndicatorConstraint>,
    one_hot_constraints: SampledCollection<crate::OneHotConstraint>,
    sos1_constraints: SampledCollection<crate::Sos1Constraint>,
    named_functions: BTreeMap<NamedFunctionID, SampledNamedFunction>,
    named_function_labels: crate::named_function::NamedFunctionLabelStore,
    sense: Option<Sense>,
}

impl SampleSetBuilder {
    /// Creates a new `SampleSetBuilder` with all fields unset.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the per-variable modeling-label store.
    pub fn variable_labels(
        mut self,
        variable_labels: crate::decision_variable::VariableLabelStore,
    ) -> Self {
        self.variable_labels = variable_labels;
        self
    }

    /// Sets the decision variables.
    pub fn decision_variables(
        mut self,
        decision_variables: BTreeMap<VariableID, SampledDecisionVariable>,
    ) -> Self {
        self.decision_variables = Some(decision_variables);
        self
    }

    /// Sets the objectives.
    pub fn objectives(mut self, objectives: Sampled<f64>) -> Self {
        self.objectives = Some(objectives);
        self
    }

    /// Sets the constraints.
    pub fn constraints(mut self, constraints: BTreeMap<ConstraintID, SampledConstraint>) -> Self {
        self.constraints = Some(
            SampledCollection::new(constraints, BTreeMap::new())
                .expect("empty removed reasons cannot reference unknown constraints"),
        );
        self
    }

    /// Sets the constraints with a full `SampledCollection` (including removed reasons).
    pub fn constraints_collection(mut self, constraints: SampledCollection<Constraint>) -> Self {
        self.constraints = Some(constraints);
        self
    }

    /// Sets the indicator constraints.
    pub fn indicator_constraints(
        mut self,
        indicator_constraints: BTreeMap<
            crate::IndicatorConstraintID,
            crate::indicator_constraint::SampledIndicatorConstraint,
        >,
    ) -> Self {
        self.indicator_constraints = SampledCollection::new(indicator_constraints, BTreeMap::new())
            .expect("empty removed reasons cannot reference unknown constraints");
        self
    }

    /// Sets the indicator constraints with a full `SampledCollection` (including removed reasons).
    pub fn indicator_constraints_collection(
        mut self,
        indicator_constraints: SampledCollection<IndicatorConstraint>,
    ) -> Self {
        self.indicator_constraints = indicator_constraints;
        self
    }

    /// Sets the one-hot constraints.
    pub fn one_hot_constraints(
        mut self,
        one_hot_constraints: BTreeMap<
            crate::OneHotConstraintID,
            crate::one_hot_constraint::SampledOneHotConstraint,
        >,
    ) -> Self {
        self.one_hot_constraints = SampledCollection::new(one_hot_constraints, BTreeMap::new())
            .expect("empty removed reasons cannot reference unknown constraints");
        self
    }

    /// Sets the one-hot constraints with a full `SampledCollection` (including removed reasons).
    pub fn one_hot_constraints_collection(
        mut self,
        one_hot_constraints: SampledCollection<crate::OneHotConstraint>,
    ) -> Self {
        self.one_hot_constraints = one_hot_constraints;
        self
    }

    /// Sets the SOS1 constraints.
    pub fn sos1_constraints(
        mut self,
        sos1_constraints: BTreeMap<
            crate::Sos1ConstraintID,
            crate::sos1_constraint::SampledSos1Constraint,
        >,
    ) -> Self {
        self.sos1_constraints = SampledCollection::new(sos1_constraints, BTreeMap::new())
            .expect("empty removed reasons cannot reference unknown constraints");
        self
    }

    /// Sets the SOS1 constraints with a full `SampledCollection` (including removed reasons).
    pub fn sos1_constraints_collection(
        mut self,
        sos1_constraints: SampledCollection<crate::Sos1Constraint>,
    ) -> Self {
        self.sos1_constraints = sos1_constraints;
        self
    }

    /// Sets the named functions.
    pub fn named_functions(
        mut self,
        named_functions: BTreeMap<NamedFunctionID, SampledNamedFunction>,
    ) -> Self {
        self.named_functions = named_functions;
        self
    }

    /// Sets the per-named-function modeling-label store.
    pub fn named_function_labels(
        mut self,
        named_function_labels: crate::named_function::NamedFunctionLabelStore,
    ) -> Self {
        self.named_function_labels = named_function_labels;
        self
    }

    /// Sets the optimization sense.
    pub fn sense(mut self, sense: Sense) -> Self {
        self.sense = Some(sense);
        self
    }

    /// Builds the `SampleSet` with validation.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Required fields (`decision_variables`, `objectives`, `constraints`, `sense`) are not set
    /// - Sample IDs are inconsistent across decision variables, objectives, constraints, and named functions
    pub fn build(self) -> Result<SampleSet, SampleSetError> {
        let decision_variables =
            self.decision_variables
                .ok_or(SampleSetError::MissingRequiredField {
                    field: "decision_variables",
                })?;
        let objectives = self
            .objectives
            .ok_or(SampleSetError::MissingRequiredField {
                field: "objectives",
            })?;
        let constraints = self
            .constraints
            .ok_or(SampleSetError::MissingRequiredField {
                field: "constraints",
            })?;
        let sense = self
            .sense
            .ok_or(SampleSetError::MissingRequiredField { field: "sense" })?;

        let decision_variable_ids = decision_variables.keys().copied().collect::<BTreeSet<_>>();
        crate::modeling_label::validate_modeling_label_ids(
            &self.variable_labels,
            &decision_variable_ids,
            "decision variable",
        )
        .map_err(|e| SampleSetError::InvalidSidecar {
            message: e.to_string(),
        })?;
        let named_function_ids = self
            .named_functions
            .keys()
            .copied()
            .collect::<BTreeSet<_>>();
        crate::modeling_label::validate_modeling_label_ids(
            &self.named_function_labels,
            &named_function_ids,
            "named function",
        )
        .map_err(|e| SampleSetError::InvalidSidecar {
            message: e.to_string(),
        })?;
        constraints
            .validate_context_ids()
            .map_err(|e| SampleSetError::InvalidSidecar {
                message: e.to_string(),
            })?;
        self.indicator_constraints
            .validate_context_ids()
            .map_err(|e| SampleSetError::InvalidSidecar {
                message: e.to_string(),
            })?;
        self.one_hot_constraints
            .validate_context_ids()
            .map_err(|e| SampleSetError::InvalidSidecar {
                message: e.to_string(),
            })?;
        self.sos1_constraints.validate_context_ids().map_err(|e| {
            SampleSetError::InvalidSidecar {
                message: e.to_string(),
            }
        })?;

        // Validate sample ID consistency
        let objective_sample_ids = objectives.ids();

        for sampled_dv in decision_variables.values() {
            if !sampled_dv.samples().has_same_ids(&objective_sample_ids) {
                return Err(SampleSetError::InconsistentSampleIDs {
                    expected: objective_sample_ids.clone(),
                    found: sampled_dv.samples().ids(),
                });
            }
        }

        constraints
            .validate_sample_ids(&objective_sample_ids)
            .map_err(|found| SampleSetError::InconsistentSampleIDs {
                expected: objective_sample_ids.clone(),
                found,
            })?;
        self.indicator_constraints
            .validate_sample_ids(&objective_sample_ids)
            .map_err(|found| SampleSetError::InconsistentSampleIDs {
                expected: objective_sample_ids.clone(),
                found,
            })?;
        self.one_hot_constraints
            .validate_sample_ids(&objective_sample_ids)
            .map_err(|found| SampleSetError::InconsistentSampleIDs {
                expected: objective_sample_ids.clone(),
                found,
            })?;
        self.sos1_constraints
            .validate_sample_ids(&objective_sample_ids)
            .map_err(|found| SampleSetError::InconsistentSampleIDs {
                expected: objective_sample_ids.clone(),
                found,
            })?;

        for sampled_named_function in self.named_functions.values() {
            if !sampled_named_function
                .evaluated_values()
                .has_same_ids(&objective_sample_ids)
            {
                return Err(SampleSetError::InconsistentSampleIDs {
                    expected: objective_sample_ids.clone(),
                    found: sampled_named_function.evaluated_values().ids(),
                });
            }
        }

        // Compute feasibility (considers both regular and indicator constraints)
        let (feasible, feasible_relaxed) = Self::compute_feasibility(
            &constraints,
            &self.indicator_constraints,
            &self.one_hot_constraints,
            &self.sos1_constraints,
            &objective_sample_ids,
        );

        Ok(SampleSet {
            decision_variables,
            variable_labels: self.variable_labels.clone(),
            objectives,
            constraints,
            indicator_constraints: self.indicator_constraints,
            one_hot_constraints: self.one_hot_constraints,
            sos1_constraints: self.sos1_constraints,
            named_functions: self.named_functions,
            named_function_labels: self.named_function_labels.clone(),
            sense,
            feasible,
            feasible_relaxed,
            metadata: Default::default(),
            annotations: Default::default(),
        })
    }

    /// Builds the `SampleSet` without invariant validation.
    ///
    /// # Safety
    /// This method does not validate that the SampleSet invariants hold.
    /// The caller must ensure:
    /// - `decision_variables` is keyed by the intended [`VariableID`] for each row
    /// - Sampled constraint collection keys and sidecars are internally consistent
    /// - Named function keys match their value's `id()`
    /// - Sample IDs are consistent across all components
    ///
    /// Use [`Self::build`] for validated construction.
    /// This method is useful when invariants are guaranteed by construction,
    /// such as when creating a SampleSet from `Instance::evaluate_samples`.
    ///
    /// # Errors
    /// Returns an error if required fields are not set.
    pub unsafe fn build_unchecked(self) -> Result<SampleSet, SampleSetError> {
        let decision_variables =
            self.decision_variables
                .ok_or(SampleSetError::MissingRequiredField {
                    field: "decision_variables",
                })?;
        let objectives = self
            .objectives
            .ok_or(SampleSetError::MissingRequiredField {
                field: "objectives",
            })?;
        let constraints = self
            .constraints
            .ok_or(SampleSetError::MissingRequiredField {
                field: "constraints",
            })?;
        let sense = self
            .sense
            .ok_or(SampleSetError::MissingRequiredField { field: "sense" })?;

        let objective_sample_ids = objectives.ids();
        let (feasible, feasible_relaxed) = Self::compute_feasibility(
            &constraints,
            &self.indicator_constraints,
            &self.one_hot_constraints,
            &self.sos1_constraints,
            &objective_sample_ids,
        );

        Ok(SampleSet {
            decision_variables,
            variable_labels: self.variable_labels.clone(),
            objectives,
            constraints,
            indicator_constraints: self.indicator_constraints,
            one_hot_constraints: self.one_hot_constraints,
            sos1_constraints: self.sos1_constraints,
            named_functions: self.named_functions,
            named_function_labels: self.named_function_labels.clone(),
            sense,
            feasible,
            feasible_relaxed,
            metadata: Default::default(),
            annotations: Default::default(),
        })
    }

    fn compute_feasibility(
        constraints: &SampledCollection<Constraint>,
        indicator_constraints: &SampledCollection<IndicatorConstraint>,
        one_hot_constraints: &SampledCollection<crate::OneHotConstraint>,
        sos1_constraints: &SampledCollection<crate::Sos1Constraint>,
        sample_ids: &SampleIDSet,
    ) -> (BTreeMap<SampleID, bool>, BTreeMap<SampleID, bool>) {
        let mut feasible = BTreeMap::new();
        let mut feasible_relaxed = BTreeMap::new();

        for sample_id in sample_ids {
            let f = constraints.is_feasible_for(*sample_id)
                && indicator_constraints.is_feasible_for(*sample_id)
                && one_hot_constraints.is_feasible_for(*sample_id)
                && sos1_constraints.is_feasible_for(*sample_id);
            let fr = constraints.is_feasible_relaxed_for(*sample_id)
                && indicator_constraints.is_feasible_relaxed_for(*sample_id)
                && one_hot_constraints.is_feasible_relaxed_for(*sample_id)
                && sos1_constraints.is_feasible_relaxed_for(*sample_id);

            feasible.insert(*sample_id, f);
            feasible_relaxed.insert(*sample_id, fr);
        }

        (feasible, feasible_relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraint::EvaluatedData;
    use crate::{
        ConstraintID, DecisionVariable, Equality, EvaluatedConstraint, SampleID,
        SampledDecisionVariable, Sense, VariableID,
    };
    use std::collections::BTreeMap;

    #[test]
    fn builder_rejects_sampled_one_hot_side_map_id_mismatch() {
        let var_id = VariableID::from(1);
        let sample_id = SampleID::from(0);
        let unexpected_sample_id = SampleID::from(1);

        let decision_variable = DecisionVariable::binary();
        let mut variable_samples = crate::Sampled::default();
        variable_samples.append([sample_id], 1.0).unwrap();
        let sampled_variable =
            SampledDecisionVariable::new(var_id, decision_variable.clone(), variable_samples)
                .unwrap();

        let mut objectives = crate::Sampled::default();
        objectives.append([sample_id], 0.0).unwrap();

        let sampled_one_hot: crate::SampledOneHotConstraint = crate::OneHotConstraint {
            variables: [var_id].into_iter().collect(),
            stage: crate::OneHotSampledData {
                feasible: BTreeMap::from([(sample_id, true)]),
                active_variable: BTreeMap::from([(unexpected_sample_id, Some(var_id))]),
                used_decision_variable_ids: [var_id].into_iter().collect(),
            },
        };
        let one_hot_constraints = crate::SampledCollection::new(
            BTreeMap::from([(crate::OneHotConstraintID::from(1), sampled_one_hot)]),
            BTreeMap::new(),
        )
        .unwrap();

        let err = SampleSet::builder()
            .decision_variables(BTreeMap::from([(var_id, sampled_variable)]))
            .objectives(objectives)
            .constraints(BTreeMap::new())
            .one_hot_constraints_collection(one_hot_constraints)
            .sense(Sense::Minimize)
            .build()
            .unwrap_err();

        assert!(
            err.to_string().contains("Inconsistent sample IDs"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn builder_rejects_orphan_variable_label_id() {
        let mut variable_labels = crate::VariableLabelStore::default();
        variable_labels.set_name(VariableID::from(99), "orphan");

        let err = SampleSet::builder()
            .decision_variables(BTreeMap::new())
            .variable_labels(variable_labels)
            .objectives(crate::Sampled::default())
            .constraints(BTreeMap::new())
            .sense(Sense::Minimize)
            .build()
            .unwrap_err();

        assert!(
            err.to_string().contains("unknown decision variable ID")
                && err.to_string().contains("VariableID(99)"),
            "unexpected error: {err}"
        );
    }

    /// Regression: `SampleSet::get(sid)` must propagate variable modeling
    /// labels, constraint label/provenance sidecars, and removed reasons into
    /// the returned per-sample `Solution`. A previous version threaded
    /// `variable_labels` only and rebuilt the constraint collections via
    /// `EvaluatedCollection::new(map, BTreeMap::new())`, silently discarding
    /// constraint labels, provenance, and relaxed-state semantics.
    #[test]
    fn test_sample_set_get_preserves_sidecars() {
        let var_id = VariableID::from(1);
        let cid = ConstraintID::from(10);
        let sample_id = SampleID::from(0);

        // Decision variable + sample
        let dv = DecisionVariable::binary();
        let mut x_samples = crate::Sampled::default();
        x_samples.append([sample_id], 1.0).unwrap();
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(
            var_id,
            SampledDecisionVariable::new(var_id, dv, x_samples).unwrap(),
        );

        let mut variable_labels = crate::VariableLabelStore::default();
        variable_labels.set_name(var_id, "x");
        variable_labels.set_subscripts(var_id, vec![0]);

        // Sampled constraint (constructed directly without going through evaluate)
        let evaluated_per_sample = EvaluatedConstraint {
            equality: Equality::EqualToZero,
            stage: EvaluatedData {
                evaluated_value: 1.0,
                dual_variable: None,
                feasible: false,
                used_decision_variable_ids: [var_id].into_iter().collect(),
            },
        };
        let mut evaluated_values = crate::Sampled::default();
        evaluated_values
            .append([sample_id], evaluated_per_sample.stage.evaluated_value)
            .unwrap();
        let mut feasible = BTreeMap::new();
        feasible.insert(sample_id, false);
        let sampled_constraint = crate::Constraint {
            equality: Equality::EqualToZero,
            stage: crate::constraint::SampledData {
                evaluated_values,
                dual_variables: None,
                feasible,
                used_decision_variable_ids: [var_id].into_iter().collect(),
            },
        };
        let mut constraints_map = BTreeMap::new();
        constraints_map.insert(cid, sampled_constraint);

        // Build a SampledCollection<Constraint> with context via builder
        let mut constraint_context = crate::ConstraintContextStore::<ConstraintID>::default();
        constraint_context.set_name(cid, "balance");
        constraint_context.set_description(cid, "demand-balance row");
        let removed_reason = crate::RemovedReason {
            reason: "relaxed for test".to_string(),
            parameters: Default::default(),
        };
        let constraints = crate::constraint_type::SampledCollection::with_context(
            constraints_map,
            BTreeMap::from([(cid, removed_reason)]),
            constraint_context,
        )
        .unwrap();

        let mut objectives = crate::Sampled::default();
        objectives.append([sample_id], 1.0).unwrap();

        let sample_set = SampleSet::builder()
            .decision_variables(decision_variables)
            .variable_labels(variable_labels)
            .objectives(objectives)
            .constraints_collection(constraints)
            .sense(Sense::Minimize)
            .build()
            .unwrap();

        assert_eq!(sample_set.is_sample_feasible(sample_id), Some(false));
        assert_eq!(sample_set.is_sample_feasible_relaxed(sample_id), Some(true));

        let solution = sample_set.get(sample_id).unwrap();
        assert!(!solution.feasible());
        assert!(solution.feasible_relaxed());
        assert!(solution.evaluated_constraints().is_removed(&cid));
        assert_eq!(solution.variable_labels().name(var_id), Some("x"));
        assert_eq!(solution.variable_labels().subscripts(var_id), &[0]);
        let constraint_meta = solution.evaluated_constraints().context();
        assert_eq!(constraint_meta.name(cid), Some("balance"));
        assert_eq!(constraint_meta.description(cid), Some("demand-balance row"));
    }
}
