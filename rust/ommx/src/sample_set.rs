mod extract;
mod parse;
mod serialize;

use crate::{
    constraint_type::SampledCollection, indicator_constraint::IndicatorConstraint, Constraint,
    ConstraintID, EvaluatedConstraint, EvaluatedDecisionVariable, EvaluatedNamedFunction,
    NamedFunctionID, SampleID, SampleIDSet, Sampled, SampledConstraint, SampledDecisionVariable,
    SampledNamedFunction, Sense, Solution, UnknownSampleIDError, VariableID,
};
use getset::Getters;
use std::collections::BTreeMap;

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

    #[error(transparent)]
    UnknownSampleIDError(#[from] UnknownSampleIDError),

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

    #[error("Decision variable key {key:?} does not match value's id {value_id:?}")]
    InconsistentDecisionVariableID {
        key: VariableID,
        value_id: VariableID,
    },

    #[error("Constraint key {key:?} does not match value's id {value_id:?}")]
    InconsistentConstraintID {
        key: ConstraintID,
        value_id: ConstraintID,
    },

    #[error("Named function key {key:?} does not match value's id {value_id:?}")]
    InconsistentNamedFunctionID {
        key: NamedFunctionID,
        value_id: NamedFunctionID,
    },
}

/// Multiple sample solution results with deduplication
///
/// Invariants
/// -----------
/// - The keys of [`Self::decision_variables`] match the `id()` of their values.
/// - The keys of [`Self::constraints`] match the `id()` of their values.
/// - The keys of [`Self::named_functions`] match the `id()` of their values.
/// - All [`Self::decision_variables`], [`Self::objectives`], [`Self::constraints`], and [`Self::named_functions`] have the same sample ID set.
/// - [`Self::feasible`] and [`Self::feasible_relaxed`] are computed from [`Self::constraints`]:
///   - `feasible`: true if all constraints are satisfied for that sample
///   - `feasible_relaxed`: true if all non-removed constraints (where `removed_reason.is_none()`) are satisfied
#[derive(Debug, Clone, Getters)]
pub struct SampleSet {
    #[getset(get = "pub")]
    decision_variables: BTreeMap<VariableID, SampledDecisionVariable>,
    #[getset(get = "pub")]
    objectives: Sampled<f64>,
    #[getset(get = "pub")]
    constraints: SampledCollection<Constraint>,
    #[getset(get = "pub")]
    indicator_constraints: SampledCollection<IndicatorConstraint>,
    #[getset(get = "pub")]
    named_functions: BTreeMap<NamedFunctionID, SampledNamedFunction>,
    #[getset(get = "pub")]
    sense: Sense,
    #[getset(get = "pub")]
    feasible: BTreeMap<SampleID, bool>,
    #[getset(get = "pub")]
    feasible_relaxed: BTreeMap<SampleID, bool>,
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

    /// Check if a specific sample is feasible
    pub fn is_sample_feasible(&self, sample_id: SampleID) -> Result<bool, UnknownSampleIDError> {
        self.feasible
            .get(&sample_id)
            .copied()
            .ok_or(UnknownSampleIDError { id: sample_id })
    }

    /// Check if a specific sample is feasible in the relaxed problem
    pub fn is_sample_feasible_relaxed(
        &self,
        sample_id: SampleID,
    ) -> Result<bool, UnknownSampleIDError> {
        self.feasible_relaxed
            .get(&sample_id)
            .copied()
            .ok_or(UnknownSampleIDError { id: sample_id })
    }

    /// Get a specific solution by sample ID
    pub fn get(&self, sample_id: crate::SampleID) -> Result<Solution, crate::UnknownSampleIDError> {
        // Get objective value
        let objective = *self.objectives.get(sample_id)?;

        // Get decision variables with substituted values - convert to EvaluatedDecisionVariable
        let mut decision_variables: BTreeMap<VariableID, EvaluatedDecisionVariable> =
            BTreeMap::default();
        for (variable_id, sampled_dv) in &self.decision_variables {
            let evaluated_dv = sampled_dv.get(sample_id)?;
            decision_variables.insert(*variable_id, evaluated_dv);
        }

        // Get evaluated constraints
        let mut evaluated_constraints: BTreeMap<ConstraintID, EvaluatedConstraint> =
            BTreeMap::default();
        for (constraint_id, constraint) in self.constraints.iter() {
            let evaluated_constraint = constraint.get(sample_id)?;
            evaluated_constraints.insert(*constraint_id, evaluated_constraint);
        }

        // Get evaluated named functions
        let mut evaluated_named_functions: BTreeMap<NamedFunctionID, EvaluatedNamedFunction> =
            BTreeMap::default();
        for (named_function_id, named_function) in &self.named_functions {
            let evaluated_named_function = named_function.get(sample_id)?;
            evaluated_named_functions.insert(*named_function_id, evaluated_named_function);
        }

        let sense = *self.sense();

        // SAFETY: SampleSet invariants guarantee Solution invariants
        Ok(unsafe {
            Solution::builder()
                .objective(objective)
                .evaluated_constraints(evaluated_constraints)
                .evaluated_named_functions(evaluated_named_functions)
                .decision_variables(decision_variables)
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
        self.get(id).map_err(SampleSetError::from)
    }

    pub fn best_feasible_relaxed(&self) -> Result<Solution, SampleSetError> {
        let id = self.best_feasible_relaxed_id()?;
        self.get(id).map_err(SampleSetError::from)
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
    objectives: Option<Sampled<f64>>,
    constraints: Option<SampledCollection<Constraint>>,
    indicator_constraints: SampledCollection<IndicatorConstraint>,
    named_functions: BTreeMap<NamedFunctionID, SampledNamedFunction>,
    sense: Option<Sense>,
}

impl SampleSetBuilder {
    /// Creates a new `SampleSetBuilder` with all fields unset.
    pub fn new() -> Self {
        Self::default()
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
        self.constraints = Some(SampledCollection::new(constraints));
        self
    }

    /// Sets the indicator constraints.
    pub fn indicator_constraints(
        mut self,
        indicator_constraints: BTreeMap<
            ConstraintID,
            crate::indicator_constraint::SampledIndicatorConstraint,
        >,
    ) -> Self {
        self.indicator_constraints = SampledCollection::new(indicator_constraints);
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
    /// - Keys do not match the `id()` of their values
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

        // Validate key/id consistency
        for (key, value) in &decision_variables {
            if key != value.id() {
                return Err(SampleSetError::InconsistentDecisionVariableID {
                    key: *key,
                    value_id: *value.id(),
                });
            }
        }

        for (key, value) in constraints.iter() {
            if *key != value.id {
                return Err(SampleSetError::InconsistentConstraintID {
                    key: *key,
                    value_id: value.id,
                });
            }
        }

        for (key, value) in &self.named_functions {
            if key != value.id() {
                return Err(SampleSetError::InconsistentNamedFunctionID {
                    key: *key,
                    value_id: *value.id(),
                });
            }
        }

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

        for sampled_constraint in constraints.values() {
            if !sampled_constraint
                .stage
                .evaluated_values
                .has_same_ids(&objective_sample_ids)
            {
                return Err(SampleSetError::InconsistentSampleIDs {
                    expected: objective_sample_ids.clone(),
                    found: sampled_constraint.stage.evaluated_values.ids(),
                });
            }
        }

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
            &objective_sample_ids,
        );

        Ok(SampleSet {
            decision_variables,
            objectives,
            constraints,
            indicator_constraints: self.indicator_constraints,
            named_functions: self.named_functions,
            sense,
            feasible,
            feasible_relaxed,
        })
    }

    /// Builds the `SampleSet` without invariant validation.
    ///
    /// # Safety
    /// This method does not validate that the SampleSet invariants hold.
    /// The caller must ensure:
    /// - Decision variable keys match their value's `id()`
    /// - Constraint keys match their value's `id()`
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
            &objective_sample_ids,
        );

        Ok(SampleSet {
            decision_variables,
            objectives,
            constraints,
            indicator_constraints: self.indicator_constraints,
            named_functions: self.named_functions,
            sense,
            feasible,
            feasible_relaxed,
        })
    }

    fn compute_feasibility(
        constraints: &SampledCollection<Constraint>,
        indicator_constraints: &SampledCollection<IndicatorConstraint>,
        sample_ids: &SampleIDSet,
    ) -> (BTreeMap<SampleID, bool>, BTreeMap<SampleID, bool>) {
        let mut feasible = BTreeMap::new();
        let mut feasible_relaxed = BTreeMap::new();

        for sample_id in sample_ids {
            let f = constraints.is_feasible_for(*sample_id)
                && indicator_constraints.is_feasible_for(*sample_id);
            let fr = constraints.is_feasible_relaxed_for(*sample_id)
                && indicator_constraints.is_feasible_relaxed_for(*sample_id);

            feasible.insert(*sample_id, f);
            feasible_relaxed.insert(*sample_id, fr);
        }

        (feasible, feasible_relaxed)
    }
}
