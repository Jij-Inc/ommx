mod approx;
mod arbitrary;
mod evaluate;
mod parse;
mod reduce_binary_power;
mod serialize;

use crate::{
    sampled::UnknownSampleIDError, Function, SampleID, Sampled, VariableID, VariableIDSet,
};
pub use arbitrary::*;
use derive_more::{Deref, From};
use fnv::{FnvHashMap, FnvHashSet};
use getset::Getters;
use std::collections::BTreeMap;

/// Constraint equality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Equality {
    /// $f(x) = 0$ type constraint.
    EqualToZero,
    /// $f(x) \leq 0$ type constraint.
    LessThanOrEqualToZero,
}

/// ID for constraint
#[derive(
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    From,
    Deref,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(transparent)]
pub struct ConstraintID(u64);

impl std::fmt::Debug for ConstraintID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ConstraintID({})", self.0)
    }
}

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

impl Constraint {
    pub fn equal_to_zero(id: ConstraintID, function: Function) -> Self {
        Self {
            id,
            function,
            equality: Equality::EqualToZero,
            name: None,
            subscripts: Vec::new(),
            parameters: FnvHashMap::default(),
            description: None,
        }
    }

    pub fn less_than_or_equal_to_zero(id: ConstraintID, function: Function) -> Self {
        Self {
            id,
            function,
            equality: Equality::LessThanOrEqualToZero,
            name: None,
            subscripts: Vec::new(),
            parameters: FnvHashMap::default(),
            description: None,
        }
    }
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
                .map(|(k, v)| format!("{k}={v}"))
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
}

/// Single evaluation result using the new design
#[derive(Debug, Clone, PartialEq, Getters)]
pub struct EvaluatedConstraint {
    #[getset(get = "pub")]
    id: ConstraintID,
    #[getset(get = "pub")]
    equality: Equality,
    #[getset(get = "pub")]
    evaluated_value: f64,
    #[getset(get = "pub")]
    feasible: bool,
    #[getset(get = "pub")]
    removed_reason: Option<String>,
    #[getset(get = "pub")]
    removed_reason_parameters: FnvHashMap<String, String>,
    #[getset(get = "pub")]
    used_decision_variable_ids: VariableIDSet,

    pub dual_variable: Option<f64>,
    pub metadata: ConstraintMetadata,
}

/// Multiple sample evaluation results with deduplication
#[derive(Debug, Clone, Getters)]
pub struct SampledConstraint {
    #[getset(get = "pub")]
    id: ConstraintID,
    #[getset(get = "pub")]
    equality: Equality,
    #[getset(get = "pub")]
    evaluated_values: Sampled<f64>,
    #[getset(get = "pub")]
    feasible: BTreeMap<SampleID, bool>,
    #[getset(get = "pub")]
    used_decision_variable_ids: VariableIDSet,
    #[getset(get = "pub")]
    removed_reason: Option<String>,
    #[getset(get = "pub")]
    removed_reason_parameters: FnvHashMap<String, String>,

    pub dual_variables: Option<Sampled<f64>>,
    pub metadata: ConstraintMetadata,
}

impl EvaluatedConstraint {
    /// Check if this constraint is feasible given a specific tolerance
    pub fn is_feasible_with_tolerance(&self, atol: crate::ATol) -> bool {
        match self.equality {
            Equality::EqualToZero => self.evaluated_value.abs() < *atol,
            Equality::LessThanOrEqualToZero => *self.evaluated_value() < *atol,
        }
    }

    /// Calculate the violation (constraint breach) value for this constraint
    ///
    /// Returns the amount by which this constraint is violated:
    /// - For `f(x) = 0`: returns `|f(x)|`
    /// - For `f(x) ≤ 0`: returns `max(0, f(x))`
    ///
    /// Returns 0.0 if the constraint is satisfied.
    pub fn violation(&self) -> f64 {
        match self.equality {
            Equality::EqualToZero => self.evaluated_value.abs(),
            Equality::LessThanOrEqualToZero => self.evaluated_value.max(0.0),
        }
    }
}

impl From<EvaluatedConstraint> for crate::v1::EvaluatedConstraint {
    fn from(constraint: EvaluatedConstraint) -> Self {
        let id = constraint.id().into_inner();
        let equality = (*constraint.equality()).into();
        let evaluated_value = *constraint.evaluated_value();
        let dual_variable = constraint.dual_variable;

        crate::v1::EvaluatedConstraint {
            id,
            equality,
            evaluated_value,
            used_decision_variable_ids: constraint
                .used_decision_variable_ids
                .into_iter()
                .map(|id| id.into_inner())
                .collect(),
            subscripts: constraint.metadata.subscripts,
            parameters: constraint.metadata.parameters.into_iter().collect(),
            name: constraint.metadata.name,
            description: constraint.metadata.description,
            dual_variable,
            removed_reason: constraint.removed_reason,
            removed_reason_parameters: constraint.removed_reason_parameters.into_iter().collect(),
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

        let feasible = *self.feasible.get(&sample_id).unwrap_or(&false);

        Ok(EvaluatedConstraint {
            id: *self.id(),
            equality: *self.equality(),
            metadata: self.metadata.clone(),
            evaluated_value,
            dual_variable,
            feasible,
            used_decision_variable_ids: self.used_decision_variable_ids.clone(),
            removed_reason: self.removed_reason().clone(),
            removed_reason_parameters: self.removed_reason_parameters().clone(),
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
        let feasible = constraint
            .feasible()
            .clone()
            .into_iter()
            .map(|(id, value)| (id.into_inner(), value))
            .collect();

        crate::v1::SampledConstraint {
            id,
            equality,
            name: constraint.metadata.name,
            subscripts: constraint.metadata.subscripts,
            parameters: constraint.metadata.parameters.into_iter().collect(),
            description: constraint.metadata.description,
            removed_reason: constraint.removed_reason,
            removed_reason_parameters: constraint.removed_reason_parameters.into_iter().collect(),
            evaluated_values: Some(evaluated_values),
            used_decision_variable_ids: constraint
                .used_decision_variable_ids
                .into_iter()
                .map(|id| id.into_inner())
                .collect(),
            feasible,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Coefficient, Evaluate};

    #[test]
    fn test_violation_equality_positive() {
        // f(x) = 0 constraint with f(x) = 2.5 → violation = |2.5| = 2.5
        let constraint = Constraint::equal_to_zero(
            ConstraintID::from(1),
            Function::Constant(Coefficient::try_from(2.5).unwrap()),
        );

        let state = crate::v1::State::default();
        let evaluated = constraint.evaluate(&state, crate::ATol::default()).unwrap();
        assert_eq!(evaluated.violation(), 2.5);
    }

    #[test]
    fn test_violation_equality_negative() {
        // f(x) = 0 constraint with f(x) = -3.0 → violation = |-3.0| = 3.0
        let constraint = Constraint::equal_to_zero(
            ConstraintID::from(1),
            Function::Constant(Coefficient::try_from(-3.0).unwrap()),
        );

        let state = crate::v1::State::default();
        let evaluated = constraint.evaluate(&state, crate::ATol::default()).unwrap();
        assert_eq!(evaluated.violation(), 3.0);
    }

    #[test]
    fn test_violation_equality_near_zero() {
        // f(x) = 0 constraint with f(x) = 0.0001 → violation = 0.0001
        // Note: Coefficient cannot be exactly 0.0
        let constraint = Constraint::equal_to_zero(
            ConstraintID::from(1),
            Function::Constant(Coefficient::try_from(0.0001).unwrap()),
        );

        let state = crate::v1::State::default();
        let evaluated = constraint.evaluate(&state, crate::ATol::default()).unwrap();
        assert_eq!(evaluated.violation(), 0.0001);
    }

    #[test]
    fn test_violation_inequality_violated() {
        // f(x) ≤ 0 constraint with f(x) = 1.5 → violation = max(0, 1.5) = 1.5
        let constraint = Constraint::less_than_or_equal_to_zero(
            ConstraintID::from(1),
            Function::Constant(Coefficient::try_from(1.5).unwrap()),
        );

        let state = crate::v1::State::default();
        let evaluated = constraint.evaluate(&state, crate::ATol::default()).unwrap();
        assert_eq!(evaluated.violation(), 1.5);
    }

    #[test]
    fn test_violation_inequality_satisfied() {
        // f(x) ≤ 0 constraint with f(x) = -1.0 → violation = max(0, -1.0) = 0.0
        let constraint = Constraint::less_than_or_equal_to_zero(
            ConstraintID::from(1),
            Function::Constant(Coefficient::try_from(-1.0).unwrap()),
        );

        let state = crate::v1::State::default();
        let evaluated = constraint.evaluate(&state, crate::ATol::default()).unwrap();
        assert_eq!(evaluated.violation(), 0.0);
    }

    #[test]
    fn test_violation_inequality_near_boundary() {
        // f(x) ≤ 0 constraint with f(x) = 0.0001 → violation = max(0, 0.0001) = 0.0001
        let constraint = Constraint::less_than_or_equal_to_zero(
            ConstraintID::from(1),
            Function::Constant(Coefficient::try_from(0.0001).unwrap()),
        );

        let state = crate::v1::State::default();
        let evaluated = constraint.evaluate(&state, crate::ATol::default()).unwrap();
        assert_eq!(evaluated.violation(), 0.0001);
    }
}
