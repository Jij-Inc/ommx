mod approx;
mod arbitrary;
mod evaluate;
mod parse;

pub use arbitrary::*;

use crate::{Function, Sampled, SampleID};
use derive_more::{Deref, From};
use fnv::FnvHashMap;

/// Constraint equality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Equality {
    /// $f(x) = 0$ type constraint.
    EqualToZero,
    /// $f(x) \leq 0$ type constraint.
    LessThanOrEqualToZero,
}

/// ID for constraint
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, From, Deref)]
pub struct ConstraintID(u64);

impl std::fmt::Display for ConstraintID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl ConstraintID {
    pub fn into_inner(self) -> u64 {
        self.0
    }
}

/// `ommx.v1.Constraint` with validated, typed fields.
#[derive(Debug, Clone, PartialEq)]
pub struct Constraint {
    pub id: ConstraintID,
    pub function: Function,
    pub equality: Equality,
    pub name: Option<String>,
    pub subscripts: Vec<i64>,
    pub parameters: FnvHashMap<String, String>,
    pub description: Option<String>,
}

impl std::fmt::Display for Constraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let equality_symbol = match self.equality {
            Equality::EqualToZero => "==",
            Equality::LessThanOrEqualToZero => "<=",
        };
        write!(f, "Constraint({} {} 0)", self.function, equality_symbol)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemovedConstraint {
    pub constraint: Constraint,
    pub removed_reason: String,
    pub removed_reason_parameters: FnvHashMap<String, String>,
}

impl std::fmt::Display for RemovedConstraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let equality_symbol = match self.constraint.equality {
            Equality::EqualToZero => "==",
            Equality::LessThanOrEqualToZero => "<=",
        };

        let mut reason_str = format!("reason={}", self.removed_reason);
        if !self.removed_reason_parameters.is_empty() {
            let params: Vec<String> = self
                .removed_reason_parameters
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect();
            reason_str = format!("{}, {}", reason_str, params.join(", "));
        }

        write!(
            f,
            "RemovedConstraint({} {} 0, {})",
            self.constraint.function, equality_symbol, reason_str
        )
    }
}

/// Core evaluation data that varies per sample
#[derive(Debug, Clone, PartialEq)]
pub struct EvaluatedConstraintCore {
    pub evaluated_value: f64,
    pub dual_variable: Option<f64>,
}

/// Shared metadata across samples
#[derive(Debug, Clone, PartialEq)]
pub struct ConstraintMetadata {
    pub id: ConstraintID,
    pub equality: Equality,
    pub name: Option<String>,
    pub subscripts: Vec<i64>,
    pub parameters: FnvHashMap<String, String>,
    pub description: Option<String>,
    pub used_decision_variable_ids: Vec<u64>,
    pub removed_reason: Option<String>,
    pub removed_reason_parameters: FnvHashMap<String, String>,
}

/// Single evaluation result using the new design
#[derive(Debug, Clone, PartialEq)]
pub struct EvaluatedConstraint {
    pub metadata: ConstraintMetadata,
    pub core: EvaluatedConstraintCore,
}

/// Multiple sample evaluation results with deduplication
#[derive(Debug, Clone)]
pub struct SampledConstraint {
    pub metadata: ConstraintMetadata,
    pub cores: Sampled<EvaluatedConstraintCore>,
    pub feasible: FnvHashMap<u64, bool>,
}

impl EvaluatedConstraint {
    /// Check if this constraint is feasible given the tolerance
    pub fn is_feasible(&self, atol: crate::ATol) -> anyhow::Result<bool> {
        Ok(match self.metadata.equality {
            Equality::EqualToZero => self.core.evaluated_value.abs() < *atol,
            Equality::LessThanOrEqualToZero => self.core.evaluated_value < *atol,
        })
    }
}

impl From<EvaluatedConstraint> for crate::v1::EvaluatedConstraint {
    fn from(constraint: EvaluatedConstraint) -> Self {
        crate::v1::EvaluatedConstraint {
            id: constraint.metadata.id.into_inner(),
            equality: constraint.metadata.equality.into(),
            evaluated_value: constraint.core.evaluated_value,
            used_decision_variable_ids: constraint.metadata.used_decision_variable_ids,
            subscripts: constraint.metadata.subscripts,
            parameters: constraint.metadata.parameters.into_iter().collect(),
            name: constraint.metadata.name,
            description: constraint.metadata.description,
            dual_variable: constraint.core.dual_variable,
            removed_reason: constraint.metadata.removed_reason,
            removed_reason_parameters: constraint.metadata.removed_reason_parameters.into_iter().collect(),
        }
    }
}

impl SampledConstraint {
    /// Get an evaluated constraint for a specific sample ID
    pub fn get(&self, sample_id: u64) -> Option<EvaluatedConstraint> {
        let target_id = SampleID::from(sample_id);
        self.cores.iter()
            .find(|(id, _)| **id == target_id)
            .map(|(_, core)| EvaluatedConstraint {
                metadata: self.metadata.clone(),
                core: core.clone(),
            })
    }

    /// Check feasibility for all samples
    pub fn is_feasible(&self, atol: crate::ATol) -> anyhow::Result<FnvHashMap<u64, bool>> {
        Ok(self.cores
            .iter()
            .map(|(sample_id, core)| {
                let feasible = match self.metadata.equality {
                    Equality::EqualToZero => core.evaluated_value.abs() < *atol,
                    Equality::LessThanOrEqualToZero => core.evaluated_value < *atol,
                };
                (sample_id.into_inner(), feasible)
            })
            .collect())
    }
}

impl From<SampledConstraint> for crate::v1::SampledConstraint {
    fn from(constraint: SampledConstraint) -> Self {
        // Convert Sampled<EvaluatedConstraintCore> to v1::SampledValues
        let sampled_values = constraint.cores.map(|core| core.evaluated_value);
        let evaluated_values: crate::v1::SampledValues = sampled_values.into();
        
        crate::v1::SampledConstraint {
            id: constraint.metadata.id.into_inner(),
            equality: constraint.metadata.equality.into(),
            name: constraint.metadata.name,
            subscripts: constraint.metadata.subscripts,
            parameters: constraint.metadata.parameters.into_iter().collect(),
            description: constraint.metadata.description,
            removed_reason: constraint.metadata.removed_reason,
            removed_reason_parameters: constraint.metadata.removed_reason_parameters.into_iter().collect(),
            evaluated_values: Some(evaluated_values),
            used_decision_variable_ids: constraint.metadata.used_decision_variable_ids,
            feasible: constraint.feasible.into_iter().collect(),
        }
    }
}
