//! Type family for constraint types.
//!
//! Each constraint type's Created form (e.g. [`Constraint`], [`IndicatorConstraint`])
//! implements [`ConstraintType`], mapping lifecycle stages to concrete types.
//!
//! This is a defunctionalization of `Stage → Type` since Rust lacks higher-kinded types.
//!
//! # Adding new constraint types
//!
//! To add a new constraint type (e.g. Disjunction, SOS1, OneHot):
//!
//! 1. Define a new struct `NewConstraint<S: Stage<Self> = Created>` with common fields
//!    (`id`, `equality`, `metadata`, `stage`) plus type-specific fields.
//! 2. Implement `Stage<NewConstraint<S>>` for each stage marker (reuse `CreatedData`,
//!    `RemovedData`, etc. if the stage data is the same as regular constraints).
//! 3. Implement `ConstraintType for NewConstraint` mapping all four stages.
//! 4. Implement `Evaluate` for `NewConstraint<Created>` and `NewConstraint<Removed>`.
//! 5. Add a `ConstraintCollection<NewConstraint>` field to [`Instance`].
//! 6. Add a variant to [`ConstraintCapability`] and update `Instance::required_capabilities`.
//!
//! [`IndicatorConstraint`]: crate::IndicatorConstraint
//! [`Instance`]: crate::Instance
//! [`ConstraintCapability`]: crate::ConstraintCapability

use crate::{
    constraint::{ConstraintID, EvaluatedConstraint, RemovedConstraint, SampledConstraint, Stage},
    v1, ATol, Constraint, Evaluate, SampleID, VariableIDSet,
};
use anyhow::Result;
use std::collections::BTreeMap;

/// A type family for constraints, mapping each lifecycle stage to a concrete type.
///
/// This trait acts as `T: Stage → Type` — a type-level function from lifecycle stages
/// to concrete constraint types. Rust lacks higher-kinded types, so we enumerate
/// the stages as associated types instead.
///
/// Each constraint kind's default (Created) form implements this trait.
/// For example, `Constraint` (= `Constraint<Created>`) implements `ConstraintType`
/// to define all stage types for regular constraints.
pub trait ConstraintType {
    /// The constraint as defined in the problem.
    type Created: Evaluate<Output = Self::Evaluated, SampledOutput = Self::Sampled>;
    /// The constraint after being removed/relaxed.
    type Removed: Evaluate<Output = Self::Evaluated, SampledOutput = Self::Sampled>;
    /// The constraint after evaluation against a single state.
    type Evaluated: EvaluatedConstraintBehavior;
    /// The constraint after evaluation against multiple samples.
    type Sampled: SampledConstraintBehavior<Evaluated = Self::Evaluated>;
}

/// Common behavior for an evaluated constraint (single state evaluation result).
pub trait EvaluatedConstraintBehavior {
    fn constraint_id(&self) -> ConstraintID;
    fn is_feasible(&self) -> bool;
    fn is_removed(&self) -> bool;
}

/// Common behavior for a sampled constraint (multi-sample evaluation result).
pub trait SampledConstraintBehavior {
    /// The evaluated constraint type returned by [`get`](Self::get).
    type Evaluated;

    fn constraint_id(&self) -> ConstraintID;
    fn is_feasible_for(&self, sample_id: SampleID) -> Option<bool>;
    fn is_removed(&self) -> bool;

    /// Extract an evaluated constraint for a specific sample.
    fn get(
        &self,
        sample_id: SampleID,
    ) -> Result<Self::Evaluated, crate::sampled::UnknownSampleIDError>;
}

// ===== Blanket-like impls for Constraint<Evaluated> and Constraint<Sampled> =====
// Both Constraint and IndicatorConstraint share EvaluatedData/SampledData in their stage,
// so the implementations are identical.

impl EvaluatedConstraintBehavior for EvaluatedConstraint {
    fn constraint_id(&self) -> ConstraintID {
        self.id
    }
    fn is_feasible(&self) -> bool {
        self.stage.feasible
    }
    fn is_removed(&self) -> bool {
        self.stage.removed_reason.is_some()
    }
}

impl SampledConstraintBehavior for SampledConstraint {
    type Evaluated = EvaluatedConstraint;

    fn constraint_id(&self) -> ConstraintID {
        self.id
    }
    fn is_feasible_for(&self, sample_id: SampleID) -> Option<bool> {
        self.stage.feasible.get(&sample_id).copied()
    }
    fn is_removed(&self) -> bool {
        self.stage.removed_reason.is_some()
    }
    fn get(
        &self,
        sample_id: SampleID,
    ) -> Result<Self::Evaluated, crate::sampled::UnknownSampleIDError> {
        // Delegate to the existing get method on Constraint<Sampled>
        SampledConstraint::get(self, sample_id)
    }
}

/// `Constraint` (= `Constraint<Created>`) serves as the type family for regular constraints.
impl ConstraintType for Constraint {
    type Created = Constraint;
    type Removed = RemovedConstraint;
    type Evaluated = EvaluatedConstraint;
    type Sampled = SampledConstraint;
}

/// A collection of active and removed constraints of the same type.
///
/// This provides the common evaluate/partial_evaluate logic
/// that Instance would otherwise duplicate for each constraint type.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstraintCollection<T: ConstraintType>
where
    T::Created: Clone + std::fmt::Debug + PartialEq,
    T::Removed: Clone + std::fmt::Debug + PartialEq,
{
    active: BTreeMap<ConstraintID, T::Created>,
    removed: BTreeMap<ConstraintID, T::Removed>,
}

impl<T: ConstraintType> Default for ConstraintCollection<T>
where
    T::Created: Clone + std::fmt::Debug + PartialEq,
    T::Removed: Clone + std::fmt::Debug + PartialEq,
{
    fn default() -> Self {
        Self {
            active: BTreeMap::new(),
            removed: BTreeMap::new(),
        }
    }
}

impl<T: ConstraintType> ConstraintCollection<T>
where
    T::Created: Clone + std::fmt::Debug + PartialEq,
    T::Removed: Clone + std::fmt::Debug + PartialEq,
{
    pub fn new(
        active: BTreeMap<ConstraintID, T::Created>,
        removed: BTreeMap<ConstraintID, T::Removed>,
    ) -> Self {
        Self { active, removed }
    }

    /// Access active constraints.
    pub fn active(&self) -> &BTreeMap<ConstraintID, T::Created> {
        &self.active
    }

    /// Access removed constraints.
    pub fn removed(&self) -> &BTreeMap<ConstraintID, T::Removed> {
        &self.removed
    }

    /// Mutable access to active constraints.
    pub fn active_mut(&mut self) -> &mut BTreeMap<ConstraintID, T::Created> {
        &mut self.active
    }

    /// Mutable access to removed constraints.
    pub fn removed_mut(&mut self) -> &mut BTreeMap<ConstraintID, T::Removed> {
        &mut self.removed
    }

    /// Consume the collection and return the active and removed maps.
    pub fn into_parts(
        self,
    ) -> (
        BTreeMap<ConstraintID, T::Created>,
        BTreeMap<ConstraintID, T::Removed>,
    ) {
        (self.active, self.removed)
    }

    pub fn is_empty(&self) -> bool {
        self.active.is_empty() && self.removed.is_empty()
    }

    /// Collect required variable IDs from all active constraints.
    pub fn required_ids(&self) -> VariableIDSet {
        let mut ids = VariableIDSet::default();
        for constraint in self.active.values() {
            ids.extend(constraint.required_ids());
        }
        ids
    }

    /// Evaluate all constraints (active and removed) against a single state.
    pub fn evaluate_all(
        &self,
        state: &v1::State,
        atol: ATol,
    ) -> Result<BTreeMap<ConstraintID, T::Evaluated>> {
        let mut results = BTreeMap::new();
        for constraint in self.active.values() {
            let evaluated = constraint.evaluate(state, atol)?;
            results.insert(evaluated.constraint_id(), evaluated);
        }
        for constraint in self.removed.values() {
            let evaluated = constraint.evaluate(state, atol)?;
            results.insert(evaluated.constraint_id(), evaluated);
        }
        Ok(results)
    }

    /// Partially evaluate all active constraints in place.
    pub fn partial_evaluate_active(&mut self, state: &v1::State, atol: ATol) -> Result<()> {
        for constraint in self.active.values_mut() {
            constraint.partial_evaluate(state, atol)?;
        }
        Ok(())
    }
}

/// A collection of evaluated constraints of a single type.
///
/// This is the Solution-side counterpart of [`ConstraintCollection`],
/// providing generic feasibility checks via [`EvaluatedConstraintBehavior`].
#[derive(Debug, Clone, PartialEq)]
pub struct EvaluatedCollection<T: ConstraintType>(BTreeMap<ConstraintID, T::Evaluated>);

impl<T: ConstraintType> std::ops::Deref for EvaluatedCollection<T> {
    type Target = BTreeMap<ConstraintID, T::Evaluated>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: ConstraintType> std::ops::DerefMut for EvaluatedCollection<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: ConstraintType> Default for EvaluatedCollection<T> {
    fn default() -> Self {
        Self(BTreeMap::new())
    }
}

impl<T: ConstraintType> EvaluatedCollection<T> {
    pub fn new(constraints: BTreeMap<ConstraintID, T::Evaluated>) -> Self {
        Self(constraints)
    }

    pub fn inner(&self) -> &BTreeMap<ConstraintID, T::Evaluated> {
        &self.0
    }

    pub fn into_inner(self) -> BTreeMap<ConstraintID, T::Evaluated> {
        self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Check if all constraints are feasible.
    pub fn is_feasible(&self) -> bool {
        self.0.values().all(|c| c.is_feasible())
    }

    /// Check if all non-removed constraints are feasible.
    pub fn is_feasible_relaxed(&self) -> bool {
        self.0
            .values()
            .filter(|c| !c.is_removed())
            .all(|c| c.is_feasible())
    }
}

/// A collection of sampled constraints of a single type.
///
/// This is the SampleSet-side counterpart of [`ConstraintCollection`],
/// providing generic per-sample feasibility checks via [`SampledConstraintBehavior`].
#[derive(Debug, Clone)]
pub struct SampledCollection<T: ConstraintType>(BTreeMap<ConstraintID, T::Sampled>);

impl<T: ConstraintType> std::ops::Deref for SampledCollection<T> {
    type Target = BTreeMap<ConstraintID, T::Sampled>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: ConstraintType> Default for SampledCollection<T> {
    fn default() -> Self {
        Self(BTreeMap::new())
    }
}

impl<T: ConstraintType> SampledCollection<T> {
    pub fn new(constraints: BTreeMap<ConstraintID, T::Sampled>) -> Self {
        Self(constraints)
    }

    pub fn inner(&self) -> &BTreeMap<ConstraintID, T::Sampled> {
        &self.0
    }

    pub fn into_inner(self) -> BTreeMap<ConstraintID, T::Sampled> {
        self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Check if all constraints are feasible for a given sample.
    pub fn is_feasible_for(&self, sample_id: SampleID) -> bool {
        self.0
            .values()
            .all(|c| c.is_feasible_for(sample_id).unwrap_or(false))
    }

    /// Check if all non-removed constraints are feasible for a given sample.
    pub fn is_feasible_relaxed_for(&self, sample_id: SampleID) -> bool {
        self.0
            .values()
            .filter(|c| !c.is_removed())
            .all(|c| c.is_feasible_for(sample_id).unwrap_or(false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, constraint::ConstraintID, linear, Function};

    #[test]
    fn constraint_type_aliases() {
        let c = Constraint::equal_to_zero(ConstraintID::from(1), Function::Zero);
        let _: <Constraint as ConstraintType>::Created = c;
    }

    #[test]
    fn empty_collection() {
        let collection = ConstraintCollection::<Constraint>::default();
        assert!(collection.is_empty());
    }

    #[test]
    fn evaluate_collection() {
        let mut active = BTreeMap::new();
        active.insert(
            ConstraintID::from(1),
            Constraint::less_than_or_equal_to_zero(
                ConstraintID::from(1),
                Function::from(linear!(1) + coeff!(-1.0)),
            ),
        );
        active.insert(
            ConstraintID::from(2),
            Constraint::equal_to_zero(
                ConstraintID::from(2),
                Function::from(linear!(1) + coeff!(-2.0)),
            ),
        );

        let collection = ConstraintCollection::<Constraint>::new(active, BTreeMap::new());

        let state = v1::State {
            entries: [(1, 1.5)].into_iter().collect(),
        };
        let results = collection.evaluate_all(&state, ATol::default()).unwrap();

        assert_eq!(results.len(), 2);
        assert!(!results[&ConstraintID::from(1)].stage.feasible);
        assert!(!results[&ConstraintID::from(2)].stage.feasible);
    }

    #[test]
    fn collection_accessors() {
        let mut active = BTreeMap::new();
        active.insert(
            ConstraintID::from(1),
            Constraint::equal_to_zero(ConstraintID::from(1), Function::Zero),
        );

        let collection = ConstraintCollection::<Constraint>::new(active, BTreeMap::new());
        assert_eq!(collection.active().len(), 1);
        assert_eq!(collection.removed().len(), 0);
    }
}
