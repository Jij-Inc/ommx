mod approx;
mod arbitrary;
mod evaluate;
mod logical_memory;
mod metadata_store;
pub mod parse;
mod reduce_binary_power;
mod serialize;
pub(crate) mod stage;

pub use metadata_store::ConstraintMetadataStore;

use crate::logical_memory::LogicalMemoryProfile;
use crate::{Function, SampleID, VariableID};
pub use arbitrary::*;
use derive_more::{Deref, From};
use fnv::{FnvHashMap, FnvHashSet};
pub use stage::{
    Created, CreatedData, Evaluated, EvaluatedData, RemovedReason, SampledData, Stage,
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

impl From<ConstraintID> for u64 {
    fn from(id: ConstraintID) -> Self {
        id.0
    }
}

/// One step in a constraint's transformation history.
///
/// For example, when an indicator constraint with indicator=1 is propagated,
/// it is promoted to a regular `Constraint` with a provenance step recording
/// the original indicator constraint ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provenance {
    IndicatorConstraint(crate::IndicatorConstraintID),
    OneHotConstraint(crate::OneHotConstraintID),
    Sos1Constraint(crate::Sos1ConstraintID),
}

/// Auxiliary metadata for constraints (excluding essential id and equality)
#[derive(Debug, Clone, PartialEq, Default, LogicalMemoryProfile)]
pub struct ConstraintMetadata {
    pub name: Option<String>,
    pub subscripts: Vec<i64>,
    pub parameters: FnvHashMap<String, String>,
    pub description: Option<String>,
    /// Chain of transformations that produced this constraint.
    ///
    /// Empty for constraints that were directly authored. When a constraint is
    /// transformed from another (e.g. an indicator constraint promoted to a
    /// regular constraint), a new [`Provenance`] entry is appended. Each entry
    /// records the identity of the constraint that existed just before the
    /// transformation. Older entries come first, newer last.
    pub provenance: Vec<Provenance>,
}

/// A constraint parameterized by its lifecycle stage.
///
/// Holds only the constraint's intrinsic data (`equality` plus stage-specific
/// data in `stage`). Auxiliary metadata (`name`, `subscripts`, `parameters`,
/// `description`, `provenance`) lives on the enclosing collection's
/// [`ConstraintMetadataStore`] keyed by id; per-element storage was retired
/// in the v3 metadata redesign.
///
/// The constraint's [`ConstraintID`] is not stored in this struct — it is
/// held by the enclosing collection (e.g. the `BTreeMap` key in
/// [`Instance`]), which is the single source of truth. Standalone
/// constraints are identity-less until inserted into a collection.
///
/// [`Instance`]: crate::Instance
/// [`ConstraintMetadataStore`]: crate::ConstraintMetadataStore
#[derive(Debug, Clone, PartialEq)]
pub struct Constraint<S: Stage<Self> = Created> {
    pub equality: Equality,
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

    pub fn equal_to_zero(function: Function) -> Self {
        Self {
            equality: Equality::EqualToZero,
            stage: CreatedData { function },
        }
    }

    pub fn less_than_or_equal_to_zero(function: Function) -> Self {
        Self {
            equality: Equality::LessThanOrEqualToZero,
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

/// Build a v1 `EvaluatedConstraint` with metadata fields defaulted.
/// Used by call sites that don't have access to the SoA store; the
/// collection-level serializer overlays metadata before emitting.
impl From<(ConstraintID, EvaluatedConstraint)> for crate::v1::EvaluatedConstraint {
    fn from((id, c): (ConstraintID, EvaluatedConstraint)) -> Self {
        evaluated_constraint_to_v1(id, c, ConstraintMetadata::default())
    }
}

impl From<(ConstraintID, SampledConstraint)> for crate::v1::SampledConstraint {
    fn from((id, c): (ConstraintID, SampledConstraint)) -> Self {
        sampled_constraint_to_v1(id, c, ConstraintMetadata::default())
    }
}

/// Build a v1 `EvaluatedConstraint` from a per-element constraint plus its
/// metadata. The metadata comes from the enclosing collection's
/// [`ConstraintMetadataStore`]; the per-element struct no longer carries it.
pub fn evaluated_constraint_to_v1(
    id: ConstraintID,
    c: EvaluatedConstraint,
    metadata: ConstraintMetadata,
) -> crate::v1::EvaluatedConstraint {
    crate::v1::EvaluatedConstraint {
        id: id.into_inner(),
        equality: c.equality.into(),
        evaluated_value: c.stage.evaluated_value,
        used_decision_variable_ids: c
            .stage
            .used_decision_variable_ids
            .into_iter()
            .map(|id| id.into_inner())
            .collect(),
        subscripts: metadata.subscripts,
        parameters: metadata.parameters.into_iter().collect(),
        name: metadata.name,
        description: metadata.description,
        dual_variable: c.stage.dual_variable,
        removed_reason: None,
        removed_reason_parameters: Default::default(),
    }
}

// ===== Sampled stage =====

/// Type alias for a sampled constraint.
pub type SampledConstraint = Constraint<stage::Sampled>;

impl SampledConstraint {
    /// Check feasibility for a specific sample.
    ///
    /// Returns [`None`] if `sample_id` is not present in the sampled data.
    pub fn is_feasible(&self, sample_id: SampleID, atol: crate::ATol) -> Option<bool> {
        let evaluated_value = *self.stage.evaluated_values.get(sample_id)?;

        Some(match self.equality {
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

/// Build a v1 `SampledConstraint` from a per-element sampled constraint plus
/// its metadata. The metadata comes from the enclosing collection's
/// [`ConstraintMetadataStore`]; the per-element struct no longer carries it.
pub fn sampled_constraint_to_v1(
    id: ConstraintID,
    c: SampledConstraint,
    metadata: ConstraintMetadata,
) -> crate::v1::SampledConstraint {
    let evaluated_values: crate::v1::SampledValues = c.stage.evaluated_values.into();
    let feasible = c
        .stage
        .feasible
        .into_iter()
        .map(|(id, value)| (id.into_inner(), value))
        .collect();

    crate::v1::SampledConstraint {
        id: id.into_inner(),
        equality: c.equality.into(),
        name: metadata.name,
        subscripts: metadata.subscripts,
        parameters: metadata.parameters.into_iter().collect(),
        description: metadata.description,
        removed_reason: None,
        removed_reason_parameters: Default::default(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Coefficient, Evaluate};

    #[test]
    fn test_violation_equality_positive() {
        let constraint =
            Constraint::equal_to_zero(Function::Constant(Coefficient::try_from(2.5).unwrap()));
        let state = crate::v1::State::default();
        let evaluated = constraint.evaluate(&state, crate::ATol::default()).unwrap();
        assert_eq!(evaluated.violation(), 2.5);
    }

    #[test]
    fn test_violation_equality_negative() {
        let constraint =
            Constraint::equal_to_zero(Function::Constant(Coefficient::try_from(-3.0).unwrap()));
        let state = crate::v1::State::default();
        let evaluated = constraint.evaluate(&state, crate::ATol::default()).unwrap();
        assert_eq!(evaluated.violation(), 3.0);
    }

    #[test]
    fn test_violation_equality_near_zero() {
        let constraint =
            Constraint::equal_to_zero(Function::Constant(Coefficient::try_from(0.0001).unwrap()));
        let state = crate::v1::State::default();
        let evaluated = constraint.evaluate(&state, crate::ATol::default()).unwrap();
        assert_eq!(evaluated.violation(), 0.0001);
    }

    #[test]
    fn test_violation_inequality_violated() {
        let constraint = Constraint::less_than_or_equal_to_zero(Function::Constant(
            Coefficient::try_from(1.5).unwrap(),
        ));
        let state = crate::v1::State::default();
        let evaluated = constraint.evaluate(&state, crate::ATol::default()).unwrap();
        assert_eq!(evaluated.violation(), 1.5);
    }

    #[test]
    fn test_violation_inequality_satisfied() {
        let constraint = Constraint::less_than_or_equal_to_zero(Function::Constant(
            Coefficient::try_from(-1.0).unwrap(),
        ));
        let state = crate::v1::State::default();
        let evaluated = constraint.evaluate(&state, crate::ATol::default()).unwrap();
        assert_eq!(evaluated.violation(), 0.0);
    }

    #[test]
    fn test_violation_inequality_near_boundary() {
        let constraint = Constraint::less_than_or_equal_to_zero(Function::Constant(
            Coefficient::try_from(0.0001).unwrap(),
        ));
        let state = crate::v1::State::default();
        let evaluated = constraint.evaluate(&state, crate::ATol::default()).unwrap();
        assert_eq!(evaluated.violation(), 0.0001);
    }
}
