mod parse;

use crate::{EvaluatedConstraint, EvaluatedDecisionVariable, Sampled, Sense};
use fnv::FnvHashMap;
use getset::Getters;

/// Single solution result with data integrity guarantees
#[derive(Debug, Clone, PartialEq, Getters)]
pub struct Solution {
    #[getset(get = "pub")]
    state: crate::v1::State,
    #[getset(get = "pub")]
    objective: f64,
    #[getset(get = "pub")]
    evaluated_constraints: Vec<EvaluatedConstraint>,
    #[getset(get = "pub")]
    decision_variables: Vec<EvaluatedDecisionVariable>,
    #[getset(get = "pub")]
    feasible: bool,
    #[getset(get = "pub")]
    feasible_relaxed: bool,
    #[getset(get = "pub")]
    optimality: crate::v1::Optimality,
    #[getset(get = "pub")]
    relaxation: crate::v1::Relaxation,
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
    feasible_relaxed: FnvHashMap<u64, bool>,
    #[getset(get = "pub")]
    feasible: FnvHashMap<u64, bool>,
    #[getset(get = "pub")]
    sense: Sense,
}

impl SampleSet {
    /// Create a new SampleSet
    pub fn new(
        decision_variables: Vec<crate::v1::SampledDecisionVariable>,
        objectives: Option<Sampled<f64>>,
        constraints: Vec<crate::SampledConstraint>,
        feasible_relaxed: FnvHashMap<u64, bool>,
        feasible: FnvHashMap<u64, bool>,
        sense: Sense,
    ) -> Self {
        Self {
            decision_variables,
            objectives,
            constraints,
            feasible_relaxed,
            feasible,
            sense,
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
        self.feasible.get(&sample_id.into_inner()).copied()
    }

    /// Check if a specific sample is feasible in the relaxed problem
    pub fn is_sample_feasible_relaxed(&self, sample_id: crate::SampleID) -> Option<bool> {
        self.feasible_relaxed.get(&sample_id.into_inner()).copied()
    }

    /// Get a specific solution by sample ID
    pub fn get(&self, sample_id: crate::SampleID) -> Result<Solution, crate::UnknownSampleIDError> {
        // Get objective value
        let objective = if let Some(objectives) = &self.objectives {
            *objectives.get(sample_id)?
        } else {
            return Err(crate::UnknownSampleIDError { id: sample_id });
        };

        // Get state from decision variables
        let mut state_entries = std::collections::HashMap::new();
        for dv in &self.decision_variables {
            if let Some(samples) = &dv.samples {
                // Convert v1::SampledValues to Sampled<f64> and get value
                let sampled: crate::Sampled<f64> = samples
                    .clone()
                    .try_into()
                    .map_err(|_| crate::UnknownSampleIDError { id: sample_id })?;
                let value = *sampled.get(sample_id)?;
                if let Some(decision_variable) = &dv.decision_variable {
                    state_entries.insert(decision_variable.id, value);
                }
            }
        }

        // Get decision variables with substituted values - convert to EvaluatedDecisionVariable
        let decision_variables: Result<Vec<_>, _> = self
            .decision_variables
            .iter()
            .filter_map(|dv| {
                dv.decision_variable.as_ref().map(|dv_def| {
                    // Parse v1::DecisionVariable to ommx::DecisionVariable
                    let parsed_dv: crate::DecisionVariable =
                        crate::Parse::parse(dv_def.clone(), &())
                            .map_err(|_| crate::UnknownSampleIDError { id: sample_id })?;

                    // Get the value for this sample
                    let value = state_entries
                        .get(&dv_def.id)
                        .copied()
                        .ok_or(crate::UnknownSampleIDError { id: sample_id })?;

                    // Create EvaluatedDecisionVariable
                    Ok(crate::EvaluatedDecisionVariable::new_internal(
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
                    ))
                })
            })
            .collect();
        let decision_variables = decision_variables?;

        let state = crate::v1::State {
            entries: state_entries,
        };

        // Get evaluated constraints
        let evaluated_constraints: Result<Vec<_>, _> =
            self.constraints.iter().map(|c| c.get(sample_id)).collect();
        let evaluated_constraints = evaluated_constraints?;

        // Get feasibility
        let feasible = *self.feasible.get(&sample_id.into_inner()).unwrap_or(&false);
        let feasible_relaxed = *self
            .feasible_relaxed
            .get(&sample_id.into_inner())
            .unwrap_or(&false);

        Ok(Solution::new(
            state,
            objective,
            evaluated_constraints,
            decision_variables,
            feasible,
            feasible_relaxed,
            crate::v1::Optimality::Unspecified,
            crate::v1::Relaxation::Unspecified,
        ))
    }
}

impl Solution {
    /// Create a new Solution
    pub fn new(
        state: crate::v1::State,
        objective: f64,
        evaluated_constraints: Vec<EvaluatedConstraint>,
        decision_variables: Vec<EvaluatedDecisionVariable>,
        feasible: bool,
        feasible_relaxed: bool,
        optimality: crate::v1::Optimality,
        relaxation: crate::v1::Relaxation,
    ) -> Self {
        Self {
            state,
            objective,
            evaluated_constraints,
            decision_variables,
            feasible,
            feasible_relaxed,
            optimality,
            relaxation,
        }
    }

    /// Get decision variable IDs used in this solution
    pub fn decision_variable_ids(&self) -> std::collections::BTreeSet<u64> {
        self.decision_variables
            .iter()
            .map(|v| v.id().into_inner())
            .collect()
    }

    /// Get constraint IDs evaluated in this solution
    pub fn constraint_ids(&self) -> std::collections::BTreeSet<crate::ConstraintID> {
        self.evaluated_constraints.iter().map(|c| *c.id()).collect()
    }

    /// Check if all constraints are feasible
    pub fn is_feasible(&self) -> bool {
        *self.feasible()
    }

    /// Check if all constraints are feasible in the relaxed problem
    pub fn is_feasible_relaxed(&self) -> bool {
        *self.feasible_relaxed()
    }
}
