//! Type family for constraint types.
//!
//! Each constraint type's Created form (e.g. [`crate::Constraint`], [`IndicatorConstraint`])
//! implements [`ConstraintType`], mapping lifecycle stages to concrete types.
//!
//! This is a defunctionalization of `Stage → Type` since Rust lacks higher-kinded types.
//!
//! # Adding new constraint types
//!
//! To add a new constraint type within the SDK (e.g. Disjunction):
//!
//! 1. Define a new struct `NewConstraint<S: Stage<Self> = Created>` with the
//!    type's intrinsic fields (`equality`, `stage`, plus anything specific to
//!    the new type). **Do not add a `context` field**: modeling labels
//!    (`name`, `subscripts`, `parameters`, `description`) and constraint
//!    transformation provenance live on the enclosing
//!    `ConstraintCollection<NewConstraint>`'s
//!    [`ConstraintContextStore`](crate::ConstraintContextStore) keyed by id.
//!    The constraint's `ConstraintID` is also held by the collection rather
//!    than the struct itself.
//! 2. Implement `Stage<NewConstraint<S>>` for each stage marker (reuse `CreatedData`,
//!    `EvaluatedData`, etc. if the stage data is the same as regular constraints).
//! 3. Implement `ConstraintType for NewConstraint` mapping its `KIND`, all three stages, and
//!    the diagnostic type. Extend the internal family dispatch and both result
//!    hosts' queries to cover the new family.
//! 4. Implement `Evaluate` for `NewConstraint<Created>`,
//!    [`EvaluatedConstraintData`] and [`SampledConstraintData`]. Every family must
//!    provide a nonnegative scalar violation and the internal `DiagnosticRow`
//!    implementation with a family-specific non-exhaustive diagnostic type.
//!    Result hosts own row selection and the values supplied to diagnostics.
//!    Feasibility is derived by the
//!    blanket behavior implementations; families cannot supply a separate rule.
//! 5. Add a `ConstraintCollection<NewConstraint>` field to [`Instance`].
//! 6. Add a variant to [`ConstraintKind`] and update
//!    `Instance::active_special_constraint_kinds`.
//! 7. Include the family in heterogeneous Python collections, name enumeration,
//!    and name-based extraction on models and results. Result extraction uses the family's
//!    violation metric; no family may be silently omitted.
//!
//! [`IndicatorConstraint`]: crate::IndicatorConstraint
//! [`Instance`]: crate::Instance
//! [`ConstraintKind`]: crate::ConstraintKind

mod diagnostics;
// Solution and SampleSet use typed dispatch without exposing their storage contracts.
pub(crate) mod dispatch;
pub(crate) use diagnostics::DiagnosticRow;

use crate::{ATol, Evaluate, ParseError, SampleID, SampleIDSet, VariableIDSet};
use std::sync::LazyLock;

static EMPTY_VARIABLE_ID_SET: LazyLock<VariableIDSet> = LazyLock::new(VariableIDSet::default);
use std::collections::BTreeMap;

#[cfg(test)]
mod violation_tests;

/// Return the sample IDs carried by a sample-keyed side map.
///
/// Sampled stage data is split across multiple per-sample side maps
/// (`selected_variable`, `indicator_on`, ...). Constraint
/// families use this helper to validate that those maps stay aligned with the
/// canonical sampled values for the same constraint.
pub(crate) fn sample_ids_from_map<V>(map: &BTreeMap<SampleID, V>) -> SampleIDSet {
    map.keys().copied().collect()
}

/// Marker trait for ID types used throughout the crate.
///
/// Every constraint and decision-variable ID newtype in `ommx`
/// (`ConstraintID`, `IndicatorConstraintID`, `OneHotConstraintID`,
/// `Sos1ConstraintID`, `VariableID`, `NamedFunctionID`) satisfies the
/// same shape: copyable, totally ordered, hashable, debuggable,
/// round-trips through `u64`, and participates in logical memory
/// profiling. Bundling those bounds into one trait removes the need
/// to repeat them at every generic site (e.g.
/// `ConstraintContextStore<ID>` and `ConstraintType::ID`).
///
/// `SampleID` is intentionally excluded: it is a sample-set index, not
/// a modeling-label-bearing entity, and does not currently impl
/// `LogicalMemoryProfile`.
///
/// A blanket impl makes any concrete type satisfying the bounds an
/// `IDType` automatically — there is nothing for callers to implement
/// manually.
pub trait IDType:
    Clone
    + Copy
    + Ord
    + std::hash::Hash
    + std::fmt::Debug
    + From<u64>
    + Into<u64>
    + crate::logical_memory::LogicalMemoryProfile
{
}

impl<T> IDType for T where
    T: Clone
        + Copy
        + Ord
        + std::hash::Hash
        + std::fmt::Debug
        + From<u64>
        + Into<u64>
        + crate::logical_memory::LogicalMemoryProfile
{
}

/// A type family for constraints, mapping each lifecycle stage to a concrete type.
///
/// This trait acts as `T: Stage → Type` — a type-level function from lifecycle stages
/// to concrete constraint types. Rust lacks higher-kinded types, so we enumerate
/// the stages as associated types instead.
///
/// Each constraint kind's default (Created) form implements this trait.
/// For example, `Constraint` (= `Constraint<Created>`) implements `ConstraintType`
/// to define all stage types for regular constraints.
/// This trait is sealed: only SDK constraint families implement it. Result hosts
/// support every such family, so generic diagnostic queries need only this bound.
/// Collection access and diagnostic queries belong to [`crate::Solution`] and
/// [`crate::SampleSet`], not to the family type.
pub trait ConstraintType: dispatch::Sealed {
    /// The runtime constraint kind represented by this type-level marker.
    ///
    /// This identifies the same kind across all lifecycle stages. It does not
    /// imply that a particular solver adapter supports the constraint.
    ///
    /// ```
    /// use ommx::{ConstraintKind, ConstraintType, Sos1Constraint};
    /// assert_eq!(Sos1Constraint::KIND, ConstraintKind::Sos1);
    /// ```
    const KIND: crate::ConstraintKind;
    /// The ID type for this constraint family.
    type ID: IDType;
    /// Family-specific detail for a violated constraint. Use a dedicated
    /// non-exhaustive type, even when empty, to allow additive information in minor releases.
    type ViolationDiagnostics;
    /// The constraint as defined in the problem.
    type Created: Evaluate<Output = Self::Evaluated, SampledOutput = Self::Sampled>
        + Clone
        + std::fmt::Debug
        + PartialEq;
    /// The constraint after evaluation against a single state.
    type Evaluated: EvaluatedConstraintBehavior<ID = Self::ID>;
    /// The constraint after evaluation against multiple samples.
    type Sampled: SampledConstraintBehavior<ID = Self::ID, Evaluated = Self::Evaluated>;
}

/// The data each constraint family must provide after a single evaluation.
///
/// `violation` is nonnegative and independent of the feasibility tolerance.
/// It is zero exactly when the constraint is satisfied without tolerance.
/// Positive infinity represents an overflowed
/// violation and is never feasible. NaN and negative values are invalid.
/// Input canonicalization belongs to the evaluation procedure, before this
/// contract applies. Each family defines its metric on the values it receives.
/// This trait does not require a stored tolerance. A family retaining a
/// tolerance-dependent decision must retain that decision's conditions itself.
pub trait EvaluatedConstraintData {
    type ID;
    fn violation(&self) -> f64;
    fn used_decision_variable_ids(&self) -> &VariableIDSet {
        &EMPTY_VARIABLE_ID_SET
    }
}

/// Feasibility derived from a constraint's scalar violation.
///
/// The blanket implementation prevents constraint families from implementing
/// a second, independent feasibility rule.
/// A finite, nonnegative violation is feasible exactly when `violation <= atol`.
pub trait EvaluatedConstraintBehavior: EvaluatedConstraintData {
    /// Compare the stored violation with `atol`, without re-evaluating inputs
    /// or changing any retained activation decisions.
    fn is_feasible(&self, atol: ATol) -> bool;
}

impl<T: EvaluatedConstraintData + ?Sized> EvaluatedConstraintBehavior for T {
    fn is_feasible(&self, atol: ATol) -> bool {
        violation_is_feasible(self.violation(), atol)
    }
}

/// Shared constraint predicate, also used while evaluating residuals and parsing
/// their wire representations across the constraint-family owner modules.
pub(crate) fn violation_is_feasible(violation: f64, atol: ATol) -> bool {
    violation >= 0.0 && atol.approx_is_zero(violation)
}

/// Validate a persisted feasibility flag against the metric restored from member values.
/// Shared by the one-hot and SOS1 wire boundaries.
pub(crate) fn validate_wire_feasibility(
    violation: f64,
    provided_feasible: bool,
    atol: ATol,
    message: &'static str,
) -> std::result::Result<(), ParseError> {
    if provided_feasible != violation_is_feasible(violation, atol) {
        return Err(ParseError::new(crate::error!(
            "Constraint feasible must equal violation <= atol"
        ))
        .context(message, "feasible"));
    }
    Ok(())
}

/// The data each constraint family must provide after sample evaluation.
pub trait SampledConstraintData {
    type ID;
    /// The evaluated constraint type returned by [`get`](Self::get).
    type Evaluated: EvaluatedConstraintBehavior<ID = Self::ID>;
    /// The same scalar metric as single-state evaluation, or `None` for a
    /// missing sample. Implementations must satisfy [`EvaluatedConstraintData`]'s
    /// nonnegativity contract.
    fn violation_for(&self, sample_id: SampleID) -> Option<f64>;

    /// Validate that every sample-keyed field inside this sampled constraint
    /// uses exactly `expected` sample IDs.
    ///
    /// Returns the first offending sample ID set when a side map is out of sync.
    fn validate_sample_ids(&self, expected: &SampleIDSet) -> std::result::Result<(), SampleIDSet>;

    /// Decision variable IDs recorded as used by this sampled constraint.
    fn used_decision_variable_ids(&self) -> &VariableIDSet {
        &EMPTY_VARIABLE_ID_SET
    }

    /// Extract an evaluated constraint for a specific sample.
    ///
    /// Returns [`None`] if `sample_id` is not present in the sampled data.
    fn get(&self, sample_id: SampleID) -> Option<Self::Evaluated>;
}

/// Common sample feasibility, derived from each sample's violation.
pub trait SampledConstraintBehavior: SampledConstraintData {
    /// Query a stored sample's violation without changing evaluation decisions.
    fn is_feasible_for(&self, sample_id: SampleID, atol: ATol) -> Option<bool>;
}

impl<T: SampledConstraintData + ?Sized> SampledConstraintBehavior for T {
    fn is_feasible_for(&self, sample_id: SampleID, atol: ATol) -> Option<bool> {
        self.violation_for(sample_id)
            .map(|violation| violation_is_feasible(violation, atol))
    }
}
