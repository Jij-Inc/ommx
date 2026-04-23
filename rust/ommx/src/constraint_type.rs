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
//!    `EvaluatedData`, etc. if the stage data is the same as regular constraints).
//! 3. Implement `ConstraintType for NewConstraint` mapping all three stages.
//! 4. Implement `Evaluate` for `NewConstraint<Created>`.
//! 5. Add a `ConstraintCollection<NewConstraint>` field to [`Instance`].
//! 6. Add a variant to [`AdditionalCapability`] and update `Instance::required_capabilities`.
//!
//! [`IndicatorConstraint`]: crate::IndicatorConstraint
//! [`Instance`]: crate::Instance
//! [`AdditionalCapability`]: crate::AdditionalCapability

use crate::Result;
use crate::{
    constraint::{ConstraintID, EvaluatedConstraint, RemovedReason, SampledConstraint},
    v1, ATol, Constraint, Evaluate, SampleID, VariableIDSet,
};
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
    /// The ID type for this constraint family.
    type ID: Clone + Copy + Ord + std::hash::Hash + std::fmt::Debug + From<u64> + Into<u64>;
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

/// Common behavior for an evaluated constraint (single state evaluation result).
pub trait EvaluatedConstraintBehavior {
    type ID;
    fn is_feasible(&self) -> bool;
}

/// Common behavior for a sampled constraint (multi-sample evaluation result).
pub trait SampledConstraintBehavior {
    type ID;
    /// The evaluated constraint type returned by [`get`](Self::get).
    type Evaluated;

    fn is_feasible_for(&self, sample_id: SampleID) -> Option<bool>;

    /// Extract an evaluated constraint for a specific sample.
    ///
    /// Returns [`None`] if `sample_id` is not present in the sampled data.
    fn get(&self, sample_id: SampleID) -> Option<Self::Evaluated>;
}

// ===== Blanket-like impls for Constraint<Evaluated> and Constraint<Sampled> =====
// Both Constraint and IndicatorConstraint share EvaluatedData/SampledData in their stage,
// so the implementations are identical.

impl EvaluatedConstraintBehavior for EvaluatedConstraint {
    type ID = ConstraintID;
    fn is_feasible(&self) -> bool {
        self.stage.feasible
    }
}

impl SampledConstraintBehavior for SampledConstraint {
    type ID = ConstraintID;
    type Evaluated = EvaluatedConstraint;

    fn is_feasible_for(&self, sample_id: SampleID) -> Option<bool> {
        self.stage.feasible.get(&sample_id).copied()
    }
    fn get(&self, sample_id: SampleID) -> Option<Self::Evaluated> {
        use crate::constraint::EvaluatedData;
        let evaluated_value = *self.stage.evaluated_values.get(sample_id)?;
        let dual_variable = self
            .stage
            .dual_variables
            .as_ref()
            .and_then(|duals| duals.get(sample_id))
            .copied();
        let feasible = *self.stage.feasible.get(&sample_id)?;

        Some(crate::Constraint {
            equality: self.equality,
            metadata: self.metadata.clone(),
            stage: EvaluatedData {
                evaluated_value,
                dual_variable,
                feasible,
                used_decision_variable_ids: self.stage.used_decision_variable_ids.clone(),
            },
        })
    }
}

/// `Constraint` (= `Constraint<Created>`) serves as the type family for regular constraints.
impl ConstraintType for Constraint {
    type ID = ConstraintID;
    type Created = Constraint;
    type Evaluated = EvaluatedConstraint;
    type Sampled = SampledConstraint;
}

/// A collection of active and removed constraints of the same type.
///
/// Removed constraints are stored as `(T::Created, RemovedReason)` pairs.
/// The `RemovedReason` is collection-level metadata, not part of the constraint itself.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstraintCollection<T: ConstraintType> {
    active: BTreeMap<T::ID, T::Created>,
    removed: BTreeMap<T::ID, (T::Created, RemovedReason)>,
}

impl<T: ConstraintType> Default for ConstraintCollection<T> {
    fn default() -> Self {
        Self {
            active: BTreeMap::new(),
            removed: BTreeMap::new(),
        }
    }
}

impl<T: ConstraintType> ConstraintCollection<T> {
    pub fn new(
        active: BTreeMap<T::ID, T::Created>,
        removed: BTreeMap<T::ID, (T::Created, RemovedReason)>,
    ) -> Self {
        Self { active, removed }
    }

    /// Access active constraints.
    pub fn active(&self) -> &BTreeMap<T::ID, T::Created> {
        &self.active
    }

    /// Access removed constraints with their removal reasons.
    pub fn removed(&self) -> &BTreeMap<T::ID, (T::Created, RemovedReason)> {
        &self.removed
    }

    /// Mutable access to active constraints.
    pub fn active_mut(&mut self) -> &mut BTreeMap<T::ID, T::Created> {
        &mut self.active
    }

    /// Mutable access to removed constraints.
    pub fn removed_mut(&mut self) -> &mut BTreeMap<T::ID, (T::Created, RemovedReason)> {
        &mut self.removed
    }

    /// Return an ID that is not used by any active or removed constraint in this collection.
    ///
    /// Returns `0` when the collection is empty, otherwise `max(existing id) + 1`.
    ///
    /// # Panics
    ///
    /// Panics if the maximum existing ID is `u64::MAX`, i.e. all IDs are exhausted.
    pub fn unused_id(&self) -> T::ID {
        let max_active = self.active.keys().last().copied().map(Into::into);
        let max_removed = self.removed.keys().last().copied().map(Into::into);
        let next = match (max_active, max_removed) {
            (None, None) => 0u64,
            (Some(a), None) => a.checked_add(1).expect("constraint ID space exhausted"),
            (None, Some(r)) => r.checked_add(1).expect("constraint ID space exhausted"),
            (Some(a), Some(r)) => a
                .max(r)
                .checked_add(1)
                .expect("constraint ID space exhausted"),
        };
        T::ID::from(next)
    }

    /// Consume the collection and return the active and removed maps.
    #[allow(clippy::type_complexity)]
    pub fn into_parts(
        self,
    ) -> (
        BTreeMap<T::ID, T::Created>,
        BTreeMap<T::ID, (T::Created, RemovedReason)>,
    ) {
        (self.active, self.removed)
    }

    /// Move an active constraint to the removed set with a reason.
    pub fn relax(&mut self, id: T::ID, removed_reason: RemovedReason) -> crate::Result<()> {
        let c = self
            .active
            .remove(&id)
            .ok_or_else(|| crate::error!("Constraint with ID {:?} not found", id))?;
        self.removed.insert(id, (c, removed_reason));
        Ok(())
    }

    /// Move a removed constraint back to the active set.
    pub fn restore(&mut self, id: T::ID) -> crate::Result<()> {
        let (constraint, _reason) = self
            .removed
            .remove(&id)
            .ok_or_else(|| crate::error!("Removed constraint with ID {:?} not found", id))?;
        self.active.insert(id, constraint);
        Ok(())
    }

    /// Collect required variable IDs from all active constraints.
    pub fn required_ids(&self) -> VariableIDSet {
        let mut ids = VariableIDSet::default();
        for constraint in self.active.values() {
            ids.extend(constraint.required_ids());
        }
        ids
    }
}

impl<T: ConstraintType> Evaluate for ConstraintCollection<T> {
    type Output = EvaluatedCollection<T>;
    type SampledOutput = SampledCollection<T>;

    fn evaluate(&self, state: &v1::State, atol: ATol) -> Result<Self::Output> {
        let mut results = BTreeMap::new();
        let mut removed_reasons = BTreeMap::new();
        for (id, constraint) in &self.active {
            let evaluated = constraint.evaluate(state, atol).inspect_err(|e| {
                tracing::error!(?id, error = %e, "failed to evaluate active constraint");
            })?;
            results.insert(*id, evaluated);
        }
        for (id, (constraint, reason)) in &self.removed {
            let evaluated = constraint.evaluate(state, atol).inspect_err(|e| {
                tracing::error!(?id, error = %e, "failed to evaluate removed constraint");
            })?;
            results.insert(*id, evaluated);
            removed_reasons.insert(*id, reason.clone());
        }
        Ok(EvaluatedCollection::new(results, removed_reasons))
    }

    fn evaluate_samples(
        &self,
        samples: &crate::Sampled<v1::State>,
        atol: ATol,
    ) -> Result<Self::SampledOutput> {
        let mut results = BTreeMap::new();
        let mut removed_reasons = BTreeMap::new();
        for (id, constraint) in &self.active {
            let evaluated = constraint.evaluate_samples(samples, atol).inspect_err(|e| {
                tracing::error!(?id, error = %e, "failed to evaluate_samples active constraint");
            })?;
            results.insert(*id, evaluated);
        }
        for (id, (constraint, reason)) in &self.removed {
            let evaluated = constraint.evaluate_samples(samples, atol).inspect_err(|e| {
                tracing::error!(?id, error = %e, "failed to evaluate_samples removed constraint");
            })?;
            results.insert(*id, evaluated);
            removed_reasons.insert(*id, reason.clone());
        }
        Ok(SampledCollection::new(results, removed_reasons))
    }

    fn partial_evaluate(&mut self, state: &v1::State, atol: ATol) -> Result<()> {
        for (id, constraint) in self.active.iter_mut() {
            constraint.partial_evaluate(state, atol).inspect_err(|e| {
                tracing::error!(?id, error = %e, "failed to partial_evaluate constraint");
            })?;
        }
        Ok(())
    }

    fn required_ids(&self) -> VariableIDSet {
        ConstraintCollection::required_ids(self)
    }
}

/// A collection of evaluated constraints of a single type.
///
/// This is the Solution-side counterpart of [`ConstraintCollection`],
/// providing generic feasibility checks via [`EvaluatedConstraintBehavior`].
#[derive(Debug, Clone, PartialEq)]
pub struct EvaluatedCollection<T: ConstraintType> {
    constraints: BTreeMap<T::ID, T::Evaluated>,
    removed_reasons: BTreeMap<T::ID, RemovedReason>,
}

impl<T: ConstraintType> std::ops::Deref for EvaluatedCollection<T> {
    type Target = BTreeMap<T::ID, T::Evaluated>;
    fn deref(&self) -> &Self::Target {
        &self.constraints
    }
}

impl<T: ConstraintType> std::ops::DerefMut for EvaluatedCollection<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.constraints
    }
}

impl<T: ConstraintType> Default for EvaluatedCollection<T> {
    fn default() -> Self {
        Self {
            constraints: BTreeMap::new(),
            removed_reasons: BTreeMap::new(),
        }
    }
}

impl<T: ConstraintType> EvaluatedCollection<T> {
    pub fn new(
        constraints: BTreeMap<T::ID, T::Evaluated>,
        removed_reasons: BTreeMap<T::ID, RemovedReason>,
    ) -> Self {
        Self {
            constraints,
            removed_reasons,
        }
    }

    pub fn inner(&self) -> &BTreeMap<T::ID, T::Evaluated> {
        &self.constraints
    }

    pub fn into_inner(self) -> BTreeMap<T::ID, T::Evaluated> {
        self.constraints
    }

    /// Access the removed reasons map.
    pub fn removed_reasons(&self) -> &BTreeMap<T::ID, RemovedReason> {
        &self.removed_reasons
    }

    /// Consume and return both the constraints and removed reasons.
    #[allow(clippy::type_complexity)]
    pub fn into_parts(
        self,
    ) -> (
        BTreeMap<T::ID, T::Evaluated>,
        BTreeMap<T::ID, RemovedReason>,
    ) {
        (self.constraints, self.removed_reasons)
    }

    /// Check if a constraint was removed.
    pub fn is_removed(&self, id: &T::ID) -> bool {
        self.removed_reasons.contains_key(id)
    }

    pub fn is_empty(&self) -> bool {
        self.constraints.is_empty()
    }

    /// Check if all constraints are feasible.
    pub fn is_feasible(&self) -> bool {
        self.constraints.values().all(|c| c.is_feasible())
    }

    /// Check if all non-removed constraints are feasible.
    pub fn is_feasible_relaxed(&self) -> bool {
        self.constraints
            .iter()
            .filter(|(id, _)| !self.removed_reasons.contains_key(id))
            .all(|(_, c)| c.is_feasible())
    }
}

/// A collection of sampled constraints of a single type.
///
/// This is the SampleSet-side counterpart of [`ConstraintCollection`],
/// providing generic per-sample feasibility checks via [`SampledConstraintBehavior`].
#[derive(Debug, Clone)]
pub struct SampledCollection<T: ConstraintType> {
    constraints: BTreeMap<T::ID, T::Sampled>,
    removed_reasons: BTreeMap<T::ID, RemovedReason>,
}

impl<T: ConstraintType> std::ops::Deref for SampledCollection<T> {
    type Target = BTreeMap<T::ID, T::Sampled>;
    fn deref(&self) -> &Self::Target {
        &self.constraints
    }
}

impl<T: ConstraintType> Default for SampledCollection<T> {
    fn default() -> Self {
        Self {
            constraints: BTreeMap::new(),
            removed_reasons: BTreeMap::new(),
        }
    }
}

impl<T: ConstraintType> SampledCollection<T> {
    pub fn new(
        constraints: BTreeMap<T::ID, T::Sampled>,
        removed_reasons: BTreeMap<T::ID, RemovedReason>,
    ) -> Self {
        Self {
            constraints,
            removed_reasons,
        }
    }

    pub fn inner(&self) -> &BTreeMap<T::ID, T::Sampled> {
        &self.constraints
    }

    pub fn into_inner(self) -> BTreeMap<T::ID, T::Sampled> {
        self.constraints
    }

    /// Access the removed reasons map.
    pub fn removed_reasons(&self) -> &BTreeMap<T::ID, RemovedReason> {
        &self.removed_reasons
    }

    /// Consume and return both the constraints and removed reasons.
    #[allow(clippy::type_complexity)]
    pub fn into_parts(self) -> (BTreeMap<T::ID, T::Sampled>, BTreeMap<T::ID, RemovedReason>) {
        (self.constraints, self.removed_reasons)
    }

    /// Check if a constraint was removed.
    pub fn is_removed(&self, id: &T::ID) -> bool {
        self.removed_reasons.contains_key(id)
    }

    pub fn is_empty(&self) -> bool {
        self.constraints.is_empty()
    }

    /// Check if all constraints are feasible for a given sample.
    pub fn is_feasible_for(&self, sample_id: SampleID) -> bool {
        self.constraints
            .values()
            .all(|c| c.is_feasible_for(sample_id).unwrap_or(false))
    }

    /// Check if all non-removed constraints are feasible for a given sample.
    pub fn is_feasible_relaxed_for(&self, sample_id: SampleID) -> bool {
        self.constraints
            .iter()
            .filter(|(id, _)| !self.removed_reasons.contains_key(id))
            .all(|(_, c)| c.is_feasible_for(sample_id).unwrap_or(false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, constraint::ConstraintID, linear, Function};

    #[test]
    fn constraint_type_aliases() {
        let c = Constraint::equal_to_zero(Function::Zero);
        let _: <Constraint as ConstraintType>::Created = c;
    }

    #[test]
    fn empty_collection() {
        let collection = ConstraintCollection::<Constraint>::default();
        assert!(collection.active().is_empty());
        assert!(collection.removed().is_empty());
    }

    #[test]
    fn evaluate_collection() {
        let mut active = BTreeMap::new();
        active.insert(
            ConstraintID::from(1),
            Constraint::less_than_or_equal_to_zero(Function::from(linear!(1) + coeff!(-1.0))),
        );
        active.insert(
            ConstraintID::from(2),
            Constraint::equal_to_zero(Function::from(linear!(1) + coeff!(-2.0))),
        );

        let collection = ConstraintCollection::<Constraint>::new(active, BTreeMap::new());

        let state = v1::State {
            entries: [(1, 1.5)].into_iter().collect(),
        };
        let results = collection.evaluate(&state, ATol::default()).unwrap();

        assert_eq!(results.len(), 2);
        assert!(!results[&ConstraintID::from(1)].stage.feasible);
        assert!(!results[&ConstraintID::from(2)].stage.feasible);
        assert!(results.removed_reasons().is_empty());
    }

    #[test]
    fn collection_accessors() {
        let mut active = BTreeMap::new();
        active.insert(
            ConstraintID::from(1),
            Constraint::equal_to_zero(Function::Zero),
        );

        let collection = ConstraintCollection::<Constraint>::new(active, BTreeMap::new());
        assert_eq!(collection.active().len(), 1);
        assert_eq!(collection.removed().len(), 0);
    }
}
