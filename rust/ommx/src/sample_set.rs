mod parse;

use crate::{
    ConstraintID, EvaluatedConstraint, EvaluatedDecisionVariable, Sampled, Sense, Solution,
    VariableID,
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
    decision_variables: Vec<crate::v1::SampledDecisionVariable>,
    #[getset(get = "pub")]
    objectives: Option<Sampled<f64>>,
    #[getset(get = "pub")]
    constraints: Vec<crate::SampledConstraint>,
    #[getset(get = "pub")]
    sense: Sense,
    /// Cached feasible values (computed from constraints or parsed from original data)
    cached_feasible: std::collections::HashMap<u64, bool>,
    /// Cached feasible_relaxed values (computed from constraints or parsed from original data)
    cached_feasible_relaxed: std::collections::HashMap<u64, bool>,
}

impl SampleSet {
    /// Create a new SampleSet
    pub fn new(
        decision_variables: Vec<crate::v1::SampledDecisionVariable>,
        objectives: Option<Sampled<f64>>,
        constraints: Vec<crate::SampledConstraint>,
        sense: Sense,
    ) -> Self {
        // Compute feasibility from constraints for all samples
        let mut cached_feasible = std::collections::HashMap::new();
        let mut cached_feasible_relaxed = std::collections::HashMap::new();
        
        // Get all sample IDs from objectives
        if let Some(objectives) = &objectives {
            for (sample_id, _) in objectives.iter() {
                let sample_id_u64 = sample_id.into_inner();
                
                // Compute feasibility from constraints
                let feasible = constraints.iter().all(|constraint| {
                    constraint.feasible().get(&sample_id_u64).copied().unwrap_or(false)
                });
                
                cached_feasible.insert(sample_id_u64, feasible);
                cached_feasible_relaxed.insert(sample_id_u64, feasible);
            }
        }
        
        Self {
            decision_variables,
            objectives,
            constraints,
            sense,
            cached_feasible,
            cached_feasible_relaxed,
        }
    }
    
    /// Create a new SampleSet with explicit feasible values (for parsing)
    pub(crate) fn new_with_feasible(
        decision_variables: Vec<crate::v1::SampledDecisionVariable>,
        objectives: Option<Sampled<f64>>,
        constraints: Vec<crate::SampledConstraint>,
        sense: Sense,
        cached_feasible: std::collections::HashMap<u64, bool>,
        cached_feasible_relaxed: std::collections::HashMap<u64, bool>,
    ) -> Self {
        Self {
            decision_variables,
            objectives,
            constraints,
            sense,
            cached_feasible,
            cached_feasible_relaxed,
        }
    }

    /// Get sample IDs available in this sample set
    pub fn sample_ids(&self) -> std::collections::BTreeSet<crate::SampleID> {
        if let Some(objectives) = &self.objectives {
            objectives.iter().map(|(id, _)| *id).collect()
        } else {
            std::collections::BTreeSet::new()
        }
    }

    /// Check if a specific sample is feasible
    pub fn is_sample_feasible(&self, sample_id: crate::SampleID) -> Option<bool> {
        self.cached_feasible.get(&sample_id.into_inner()).copied()
    }

    /// Check if a specific sample is feasible in the relaxed problem
    pub fn is_sample_feasible_relaxed(&self, sample_id: crate::SampleID) -> Option<bool> {
        self.cached_feasible_relaxed.get(&sample_id.into_inner()).copied()
    }

    /// Get a specific solution by sample ID
    pub fn get(&self, sample_id: crate::SampleID) -> Result<Solution, crate::UnknownSampleIDError> {
        // Get objective value
        let objective = if let Some(objectives) = &self.objectives {
            *objectives.get(sample_id)?
        } else {
            return Err(crate::UnknownSampleIDError { id: sample_id });
        };

        // Get decision variables with substituted values - convert to EvaluatedDecisionVariable
        let mut decision_variables: BTreeMap<VariableID, EvaluatedDecisionVariable> =
            BTreeMap::default();
        for dv in &self.decision_variables {
            if let Some(samples) = &dv.samples {
                if let Some(decision_variable) = &dv.decision_variable {
                    // Parse v1::DecisionVariable to ommx::DecisionVariable
                    let parsed_dv: crate::DecisionVariable =
                        crate::Parse::parse(decision_variable.clone(), &())
                            .map_err(|_| crate::UnknownSampleIDError { id: sample_id })?;

                    // Convert v1::SampledValues to Sampled<f64> and get value
                    let sampled: crate::Sampled<f64> = samples
                        .clone()
                        .try_into()
                        .map_err(|_| crate::UnknownSampleIDError { id: sample_id })?;
                    let value = *sampled.get(sample_id)?;

                    // Create EvaluatedDecisionVariable
                    let evaluated_dv = crate::EvaluatedDecisionVariable::new_internal(
                        parsed_dv.id(),
                        parsed_dv.kind(),
                        parsed_dv.bound(),
                        value,
                        crate::DecisionVariableMetadata {
                            name: parsed_dv.name.clone(),
                            subscripts: parsed_dv.subscripts.clone(),
                            parameters: parsed_dv.parameters.clone(),
                            description: parsed_dv.description.clone(),
                        },
                    );
                    decision_variables.insert(parsed_dv.id(), evaluated_dv);
                }
            }
        }

        // Get evaluated constraints
        let mut evaluated_constraints: BTreeMap<ConstraintID, EvaluatedConstraint> =
            BTreeMap::default();
        for constraint in &self.constraints {
            let evaluated_constraint = constraint.get(sample_id)?;
            evaluated_constraints.insert(*evaluated_constraint.id(), evaluated_constraint);
        }

        Ok(Solution::new(
            objective,
            evaluated_constraints,
            decision_variables,
        ))
    }
}
