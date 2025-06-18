mod approx;
mod arbitrary;
mod evaluate;
mod parse;

pub use arbitrary::*;

use crate::{sampled::UnknownSampleIDError, Function, SampleID, Sampled};
use derive_more::{Deref, From};
use fnv::{FnvHashMap, FnvHashSet};
use getset::Getters;

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

/// Auxiliary metadata for constraints (excluding essential id and equality)
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ConstraintMetadata {
    pub name: Option<String>,
    pub subscripts: Vec<i64>,
    pub parameters: FnvHashMap<String, String>,
    pub description: Option<String>,
    pub used_decision_variable_ids: Vec<u64>,
    pub removed_reason: Option<String>,
    pub removed_reason_parameters: FnvHashMap<String, String>,
}

/// Single evaluation result using the new design
#[derive(Debug, Clone, PartialEq, Getters)]
pub struct EvaluatedConstraint {
    #[getset(get = "pub")]
    id: ConstraintID,
    #[getset(get = "pub")]
    equality: Equality,
    pub metadata: ConstraintMetadata,
    #[getset(get = "pub")]
    evaluated_value: f64,
    #[getset(get = "pub")]
    dual_variable: Option<f64>,
    #[getset(get = "pub")]
    feasible: bool,
}

/// Multiple sample evaluation results with deduplication
#[derive(Debug, Clone, Getters)]
pub struct SampledConstraint {
    #[getset(get = "pub")]
    id: ConstraintID,
    #[getset(get = "pub")]
    equality: Equality,
    pub metadata: ConstraintMetadata,
    #[getset(get = "pub")]
    evaluated_values: Sampled<f64>,
    #[getset(get = "pub")]
    dual_variables: Option<Sampled<f64>>,
    #[getset(get = "pub")]
    feasible: FnvHashMap<u64, bool>,
}

impl EvaluatedConstraint {
    /// Check if this constraint is feasible given a specific tolerance
    pub fn is_feasible_with_tolerance(&self, atol: crate::ATol) -> bool {
        match self.equality {
            Equality::EqualToZero => self.evaluated_value.abs() < *atol,
            Equality::LessThanOrEqualToZero => *self.evaluated_value() < *atol,
        }
    }
}

impl From<EvaluatedConstraint> for crate::v1::EvaluatedConstraint {
    fn from(constraint: EvaluatedConstraint) -> Self {
        let id = constraint.id().into_inner();
        let equality = (*constraint.equality()).into();
        let evaluated_value = *constraint.evaluated_value();
        let dual_variable = *constraint.dual_variable();

        crate::v1::EvaluatedConstraint {
            id,
            equality,
            evaluated_value,
            used_decision_variable_ids: constraint.metadata.used_decision_variable_ids,
            subscripts: constraint.metadata.subscripts,
            parameters: constraint.metadata.parameters.into_iter().collect(),
            name: constraint.metadata.name,
            description: constraint.metadata.description,
            dual_variable,
            removed_reason: constraint.metadata.removed_reason,
            removed_reason_parameters: constraint
                .metadata
                .removed_reason_parameters
                .into_iter()
                .collect(),
        }
    }
}

impl SampledConstraint {
    /// Get an evaluated constraint for a specific sample ID
    pub fn get(&self, sample_id: SampleID) -> Result<EvaluatedConstraint, UnknownSampleIDError> {
        let evaluated_value = *self.evaluated_values.get(sample_id)?;

        let dual_variable = self
            .dual_variables
            .as_ref()
            .and_then(|duals| duals.get(sample_id).ok())
            .copied();

        let feasible = *self.feasible.get(&sample_id.into_inner()).unwrap_or(&false);

        Ok(EvaluatedConstraint {
            id: *self.id(),
            equality: *self.equality(),
            metadata: self.metadata.clone(),
            evaluated_value,
            dual_variable,
            feasible,
        })
    }

    /// Check feasibility for a specific sample
    pub fn is_feasible(
        &self,
        sample_id: SampleID,
        atol: crate::ATol,
    ) -> Result<bool, UnknownSampleIDError> {
        let evaluated_value = *self.evaluated_values.get(sample_id)?;

        Ok(match *self.equality() {
            Equality::EqualToZero => evaluated_value.abs() < *atol,
            Equality::LessThanOrEqualToZero => evaluated_value < *atol,
        })
    }

    /// Get all sample IDs that are feasible
    pub fn feasible_ids(&self, atol: crate::ATol) -> FnvHashSet<SampleID> {
        self.evaluated_values()
            .iter()
            .filter_map(|(sample_id, evaluated_value)| {
                let feasible = match *self.equality() {
                    Equality::EqualToZero => evaluated_value.abs() < *atol,
                    Equality::LessThanOrEqualToZero => *evaluated_value < *atol,
                };
                if feasible {
                    Some(*sample_id)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get all sample IDs that are infeasible
    pub fn infeasible_ids(&self, atol: crate::ATol) -> FnvHashSet<SampleID> {
        self.evaluated_values()
            .iter()
            .filter_map(|(sample_id, evaluated_value)| {
                let feasible = match *self.equality() {
                    Equality::EqualToZero => evaluated_value.abs() < *atol,
                    Equality::LessThanOrEqualToZero => *evaluated_value < *atol,
                };
                if !feasible {
                    Some(*sample_id)
                } else {
                    None
                }
            })
            .collect()
    }
}

impl From<SampledConstraint> for crate::v1::SampledConstraint {
    fn from(constraint: SampledConstraint) -> Self {
        // Convert Sampled<f64> to v1::SampledValues
        let evaluated_values: crate::v1::SampledValues =
            constraint.evaluated_values().clone().into();
        let id = constraint.id().into_inner();
        let equality = (*constraint.equality()).into();
        let feasible = constraint.feasible().clone().into_iter().collect();

        crate::v1::SampledConstraint {
            id,
            equality,
            name: constraint.metadata.name,
            subscripts: constraint.metadata.subscripts,
            parameters: constraint.metadata.parameters.into_iter().collect(),
            description: constraint.metadata.description,
            removed_reason: constraint.metadata.removed_reason,
            removed_reason_parameters: constraint
                .metadata
                .removed_reason_parameters
                .into_iter()
                .collect(),
            evaluated_values: Some(evaluated_values),
            used_decision_variable_ids: constraint.metadata.used_decision_variable_ids,
            feasible,
        }
    }
}
