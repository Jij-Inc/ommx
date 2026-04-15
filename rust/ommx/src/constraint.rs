mod approx;
mod arbitrary;
mod evaluate;
mod logical_memory;
mod parse;
mod reduce_binary_power;
mod serialize;
pub(crate) mod stage;

use crate::{sampled::UnknownSampleIDError, Function, SampleID, VariableID};
pub use arbitrary::*;
use derive_more::{Deref, From};
use fnv::{FnvHashMap, FnvHashSet};
pub use stage::{
    Created, CreatedData, Evaluated, EvaluatedData, Removed, RemovedData, RemovedReason,
    SampledData, Stage,
};
// Note: stage::Sampled is NOT re-exported here to avoid name collision
// with crate::Sampled<T> (the sampled values type). Use constraint::stage::Sampled
// or the SampledConstraint type alias instead.

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

/// Auxiliary metadata for constraints (excluding essential id and equality)
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ConstraintMetadata {
    pub name: Option<String>,
    pub subscripts: Vec<i64>,
    pub parameters: FnvHashMap<String, String>,
    pub description: Option<String>,
}

/// A constraint parameterized by its lifecycle stage.
///
/// Common fields (`id`, `equality`, `metadata`) are always present.
/// Stage-specific data (e.g. `function` for [`Created`], evaluation results for [`Evaluated`])
/// is stored in the `stage` field, whose type is determined by `S::Data`.
#[derive(Debug, Clone, PartialEq)]
pub struct Constraint<S: Stage<Self> = Created> {
    pub id: ConstraintID,
    pub equality: Equality,
    pub metadata: ConstraintMetadata,
    pub stage: S::Data,
}

// ===== Created stage (the "definition" form) =====

impl Constraint<Created> {
    /// Access the constraint function.
    pub fn function(&self) -> &Function {
        &self.stage.function
    }

    /// Mutable access to the constraint function.
    pub fn function_mut(&mut self) -> &mut Function {
        &mut self.stage.function
    }

    pub fn equal_to_zero(id: ConstraintID, function: Function) -> Self {
        Self {
            id,
            equality: Equality::EqualToZero,
            metadata: ConstraintMetadata::default(),
            stage: CreatedData { function },
        }
    }

    pub fn less_than_or_equal_to_zero(id: ConstraintID, function: Function) -> Self {
        Self {
            id,
            equality: Equality::LessThanOrEqualToZero,
            metadata: ConstraintMetadata::default(),
            stage: CreatedData { function },
        }
    }
}

impl std::fmt::Display for Constraint<Created> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let equality_symbol = match self.equality {
            Equality::EqualToZero => "==",
            Equality::LessThanOrEqualToZero => "<=",
        };
        write!(
            f,
            "Constraint({} {} 0)",
            self.stage.function, equality_symbol
        )
    }
}

// ===== Removed stage =====

/// Type alias for a removed constraint.
pub type RemovedConstraint = Constraint<Removed>;

impl RemovedConstraint {
    /// Access the constraint function.
    pub fn function(&self) -> &Function {
        &self.stage.function
    }
}

impl std::fmt::Display for RemovedConstraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let equality_symbol = match self.equality {
            Equality::EqualToZero => "==",
            Equality::LessThanOrEqualToZero => "<=",
        };

        let mut reason_str = format!("reason={}", self.stage.removed_reason.reason);
        if !self.stage.removed_reason.parameters.is_empty() {
            let params: Vec<String> = self
                .stage
                .removed_reason
                .parameters
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect();
            reason_str = format!("{}, {}", reason_str, params.join(", "));
        }

        write!(
            f,
            "RemovedConstraint({} {} 0, {})",
            self.stage.function, equality_symbol, reason_str
        )
    }
}

// ===== Evaluated stage =====

/// Type alias for an evaluated constraint.
pub type EvaluatedConstraint = Constraint<Evaluated>;

impl EvaluatedConstraint {
    /// Check if this constraint is feasible given a specific tolerance
    pub fn is_feasible_with_tolerance(&self, atol: crate::ATol) -> bool {
        match self.equality {
            Equality::EqualToZero => self.stage.evaluated_value.abs() < *atol,
            Equality::LessThanOrEqualToZero => self.stage.evaluated_value < *atol,
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
            Equality::EqualToZero => self.stage.evaluated_value.abs(),
            Equality::LessThanOrEqualToZero => self.stage.evaluated_value.max(0.0),
        }
    }
}

impl From<EvaluatedConstraint> for crate::v1::EvaluatedConstraint {
    fn from(c: EvaluatedConstraint) -> Self {
        crate::v1::EvaluatedConstraint {
            id: c.id.into_inner(),
            equality: c.equality.into(),
            evaluated_value: c.stage.evaluated_value,
            used_decision_variable_ids: c
                .stage
                .used_decision_variable_ids
                .into_iter()
                .map(|id| id.into_inner())
                .collect(),
            subscripts: c.metadata.subscripts,
            parameters: c.metadata.parameters.into_iter().collect(),
            name: c.metadata.name,
            description: c.metadata.description,
            dual_variable: c.stage.dual_variable,
            removed_reason: c.stage.removed_reason.as_ref().map(|r| r.reason.clone()),
            removed_reason_parameters: c
                .stage
                .removed_reason
                .map(|r| r.parameters.into_iter().collect())
                .unwrap_or_default(),
        }
    }
}

// ===== Sampled stage =====

/// Type alias for a sampled constraint.
pub type SampledConstraint = Constraint<stage::Sampled>;

impl SampledConstraint {
    /// Check feasibility for a specific sample
    pub fn is_feasible(
        &self,
        sample_id: SampleID,
        atol: crate::ATol,
    ) -> Result<bool, UnknownSampleIDError> {
        let evaluated_value = *self.stage.evaluated_values.get(sample_id)?;

        Ok(match self.equality {
            Equality::EqualToZero => evaluated_value.abs() < *atol,
            Equality::LessThanOrEqualToZero => evaluated_value < *atol,
        })
    }

    /// Get all sample IDs that are feasible
    pub fn feasible_ids(&self, atol: crate::ATol) -> FnvHashSet<SampleID> {
        self.stage
            .evaluated_values
            .iter()
            .filter_map(|(sample_id, evaluated_value)| {
                let feasible = match self.equality {
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
        self.stage
            .evaluated_values
            .iter()
            .filter_map(|(sample_id, evaluated_value)| {
                let feasible = match self.equality {
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
    fn from(c: SampledConstraint) -> Self {
        let evaluated_values: crate::v1::SampledValues = c.stage.evaluated_values.into();
        let feasible = c
            .stage
            .feasible
            .into_iter()
            .map(|(id, value)| (id.into_inner(), value))
            .collect();

        crate::v1::SampledConstraint {
            id: c.id.into_inner(),
            equality: c.equality.into(),
            name: c.metadata.name,
            subscripts: c.metadata.subscripts,
            parameters: c.metadata.parameters.into_iter().collect(),
            description: c.metadata.description,
            removed_reason: c.stage.removed_reason.as_ref().map(|r| r.reason.clone()),
            removed_reason_parameters: c
                .stage
                .removed_reason
                .map(|r| r.parameters.into_iter().collect())
                .unwrap_or_default(),
            evaluated_values: Some(evaluated_values),
            used_decision_variable_ids: c
                .stage
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
        let constraint = Constraint::less_than_or_equal_to_zero(
            ConstraintID::from(1),
            Function::Constant(Coefficient::try_from(0.0001).unwrap()),
        );
        let state = crate::v1::State::default();
        let evaluated = constraint.evaluate(&state, crate::ATol::default()).unwrap();
        assert_eq!(evaluated.violation(), 0.0001);
    }
}
