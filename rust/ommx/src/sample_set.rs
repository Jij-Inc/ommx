mod parse;

use crate::{
    ConstraintID, EvaluatedConstraint, EvaluatedDecisionVariable, SampleID, Sampled,
    SampledConstraint, SampledDecisionVariable, Sense, Solution, VariableID,
};
use getset::Getters;
use std::collections::BTreeMap;

/// Error occurred during SampleSet validation
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
}

/// Multiple sample solution results with deduplication
#[derive(Debug, Clone, Getters)]
pub struct SampleSet {
    #[getset(get = "pub")]
    decision_variables: BTreeMap<VariableID, SampledDecisionVariable>,
    #[getset(get = "pub")]
    objectives: Sampled<f64>,
    #[getset(get = "pub")]
    constraints: BTreeMap<ConstraintID, SampledConstraint>,
    #[getset(get = "pub")]
    sense: Sense,
    /// Feasible values (computed from constraints or parsed from original data)
    feasible: BTreeMap<SampleID, bool>,
    /// Feasible relaxed values (computed from constraints or parsed from original data)
    feasible_relaxed: BTreeMap<SampleID, bool>,
}

impl SampleSet {
    /// Create a new SampleSet
    pub fn new(
        decision_variables: BTreeMap<VariableID, SampledDecisionVariable>,
        objectives: Sampled<f64>,
        constraints: BTreeMap<ConstraintID, SampledConstraint>,
        sense: Sense,
    ) -> Self {
        // Compute feasibility from constraints for all samples
        let mut feasible = BTreeMap::new();
        let mut feasible_relaxed = BTreeMap::new();

        // Get all sample IDs from objectives
        for (sample_id, _) in objectives.iter() {
            // Compute feasibility from constraints
            let is_feasible = constraints.values().all(|constraint| {
                constraint
                    .feasible()
                    .get(&sample_id.into_inner())
                    .copied()
                    .unwrap_or(false)
            });

            feasible.insert(*sample_id, is_feasible);
            feasible_relaxed.insert(*sample_id, is_feasible);
        }

        Self {
            decision_variables,
            objectives,
            constraints,
            sense,
            feasible,
            feasible_relaxed,
        }
    }

    /// Create a new SampleSet with explicit feasible values (for parsing)
    pub(crate) fn new_with_feasible(
        decision_variables: BTreeMap<VariableID, SampledDecisionVariable>,
        objectives: Sampled<f64>,
        constraints: BTreeMap<ConstraintID, SampledConstraint>,
        sense: Sense,
        feasible: BTreeMap<SampleID, bool>,
        feasible_relaxed: BTreeMap<SampleID, bool>,
    ) -> Self {
        Self {
            decision_variables,
            objectives,
            constraints,
            sense,
            feasible,
            feasible_relaxed,
        }
    }

    /// Get sample IDs available in this sample set
    pub fn sample_ids(&self) -> std::collections::BTreeSet<crate::SampleID> {
        self.objectives.iter().map(|(id, _)| *id).collect()
    }

    /// Check if a specific sample is feasible
    pub fn is_sample_feasible(&self, sample_id: crate::SampleID) -> Option<bool> {
        self.feasible.get(&sample_id).copied()
    }

    /// Check if a specific sample is feasible in the relaxed problem
    pub fn is_sample_feasible_relaxed(&self, sample_id: crate::SampleID) -> Option<bool> {
        self.feasible_relaxed.get(&sample_id).copied()
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
        for (constraint_id, constraint) in &self.constraints {
            let evaluated_constraint = constraint.get(sample_id)?;
            evaluated_constraints.insert(*constraint_id, evaluated_constraint);
        }

        Ok(Solution::new(
            objective,
            evaluated_constraints,
            decision_variables,
        ))
    }
}
