mod parse;

use crate::{EvaluatedConstraint, Sampled, Sense};
use fnv::FnvHashMap;
use getset::Getters;

/// Auxiliary metadata for solutions (excluding essential evaluation results)
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SolutionMetadata {
    pub optimality: crate::v1::Optimality,
    pub relaxation: crate::v1::Relaxation,
    pub feasible_unrelaxed: bool, // Deprecated but maintained for compatibility
}

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
    decision_variables: Vec<crate::v1::DecisionVariable>,
    #[getset(get = "pub")]
    feasible: bool,
    #[getset(get = "pub")]
    feasible_relaxed: bool,
    pub metadata: SolutionMetadata,
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

impl Solution {
    /// Create a new Solution
    pub fn new(
        state: crate::v1::State,
        objective: f64,
        evaluated_constraints: Vec<EvaluatedConstraint>,
        decision_variables: Vec<crate::v1::DecisionVariable>,
        feasible: bool,
        feasible_relaxed: bool,
        metadata: SolutionMetadata,
    ) -> Self {
        Self {
            state,
            objective,
            evaluated_constraints,
            decision_variables,
            feasible,
            feasible_relaxed,
            metadata,
        }
    }

    /// Get decision variable IDs used in this solution
    pub fn decision_variable_ids(&self) -> std::collections::BTreeSet<u64> {
        self.decision_variables.iter().map(|v| v.id).collect()
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

    /// Get all sample IDs in this sample set
    pub fn sample_ids(&self) -> std::collections::BTreeSet<crate::SampleID> {
        let mut ids = std::collections::BTreeSet::new();
        
        // Collect from objectives if present
        if let Some(objectives) = &self.objectives {
            for (sample_id, _) in objectives.iter() {
                ids.insert(*sample_id);
            }
        }
        
        // Collect from feasible maps
        for &sample_id in self.feasible.keys() {
            ids.insert(crate::SampleID::from(sample_id));
        }
        
        ids
    }

    /// Check if a specific sample is feasible
    pub fn is_sample_feasible(&self, sample_id: crate::SampleID) -> Option<bool> {
        self.feasible.get(&sample_id.into_inner()).copied()
    }

    /// Check if a specific sample is feasible in the relaxed problem
    pub fn is_sample_feasible_relaxed(&self, sample_id: crate::SampleID) -> Option<bool> {
        self.feasible_relaxed.get(&sample_id.into_inner()).copied()
    }
}
