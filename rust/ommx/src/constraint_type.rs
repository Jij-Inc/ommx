//! Type family for constraint types.
//!
//! Each constraint type's Created form (e.g. [`Constraint`], [`IndicatorConstraint`])
//! implements [`ConstraintType`], mapping lifecycle stages to concrete types.
//!
//! This is a defunctionalization of `Stage → Type` since Rust lacks higher-kinded types.
//!
//! # Adding new constraint types
//!
//! To add a new constraint type (e.g. Disjunction):
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
    constraint::{
        ConstraintContext, ConstraintContextStore, ConstraintID, EvaluatedConstraint,
        RemovedReason, SampledConstraint,
    },
    v1, ATol, Constraint, Evaluate, SampleID, SampleIDSet, VariableIDSet,
};
use std::collections::{BTreeMap, BTreeSet};

fn validate_no_key_overlap<ID, L, R>(
    left: &BTreeMap<ID, L>,
    right: &BTreeMap<ID, R>,
    left_name: &str,
    right_name: &str,
) -> crate::Result<()>
where
    ID: IDType,
{
    if let Some(id) = left.keys().find(|id| right.contains_key(id)) {
        crate::bail!(
            { ?id },
            "Constraint ID {id:?} appears in both {left_name} and {right_name}",
        );
    }
    Ok(())
}

fn validate_removed_reasons_reference_entries<ID, V>(
    constraints: &BTreeMap<ID, V>,
    removed_reasons: &BTreeMap<ID, RemovedReason>,
) -> crate::Result<()>
where
    ID: IDType,
{
    if let Some(id) = removed_reasons
        .keys()
        .find(|id| !constraints.contains_key(id))
    {
        crate::bail!({ ?id }, "Removed reason references unknown constraint ID {id:?}");
    }
    Ok(())
}

fn validate_context_reference_ids<ID>(
    context: &ConstraintContextStore<ID>,
    owned_ids: &BTreeSet<ID>,
) -> crate::Result<()>
where
    ID: IDType,
{
    if let Some(id) = context.ids().into_iter().find(|id| !owned_ids.contains(id)) {
        crate::bail!(
            { ?id },
            "Constraint label/provenance references unknown constraint ID {id:?}",
        );
    }
    Ok(())
}

/// Return the sample IDs carried by a sample-keyed side map.
///
/// Sampled stage data is split across multiple per-sample side maps
/// (`feasible`, `active_variable`, `indicator_active`, ...). Constraint
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
pub trait ConstraintType {
    /// The ID type for this constraint family.
    type ID: IDType;
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

    /// Validate that every sample-keyed field inside this sampled constraint
    /// uses exactly `expected` sample IDs.
    ///
    /// Returns the first offending sample ID set when a side map is out of sync.
    fn validate_sample_ids(&self, expected: &SampleIDSet) -> std::result::Result<(), SampleIDSet>;

    /// Decision variable IDs recorded as used by this sampled constraint.
    fn used_decision_variable_ids(&self) -> &VariableIDSet;

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

    fn validate_sample_ids(&self, expected: &SampleIDSet) -> std::result::Result<(), SampleIDSet> {
        if !self.stage.evaluated_values.has_same_ids(expected) {
            return Err(self.stage.evaluated_values.ids());
        }
        let feasible_ids = sample_ids_from_map(&self.stage.feasible);
        if &feasible_ids != expected {
            return Err(feasible_ids);
        }
        if let Some(dual_variables) = &self.stage.dual_variables {
            if !dual_variables.has_same_ids(expected) {
                return Err(dual_variables.ids());
            }
        }
        Ok(())
    }

    fn used_decision_variable_ids(&self) -> &VariableIDSet {
        &self.stage.used_decision_variable_ids
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
/// The `RemovedReason` is removal state, not part of the constraint itself.
///
/// Per-constraint modeling labels and transformation provenance are held by
/// [`Self::context`] in Struct-of-Arrays form keyed by `T::ID`. The store
/// rides through to [`EvaluatedCollection`] / [`SampledCollection`] on
/// evaluation, so the modeling, Solution, and SampleSet layers all read from
/// one canonical sidecar source per collection.
///
/// This collection owns the table-level invariants for one constraint family:
///
/// - active and removed IDs are disjoint;
/// - removed reasons exist only for removed constraints;
/// - every label/provenance sidecar ID belongs to either an active or removed
///   constraint in this collection.
///
/// Host objects such as [`crate::Instance`] and [`crate::ParametricInstance`]
/// still own cross-table semantic invariants, including referenced
/// decision-variable IDs and special-constraint structural requirements.
///
/// # Family-local operations
///
/// Mathematically, this is one constraint-family component
/// `C_tau = Active_tau + Removed_tau + Context_tau` of an enclosing instance.
/// It supports only family-local row effects:
///
/// - construction from active rows, removed rows, and context;
/// - read access to active rows, removed rows, and context;
/// - fresh active-row insertion together with context;
/// - lifecycle-preserving row replacement after host validation;
/// - by-value active-row rewrites that either keep rows active or move them to
///   removed with a host-supplied reason;
/// - active-to-removed lifecycle movement;
/// - restore through a host-supplied normalizer;
/// - context updates for IDs owned by this collection;
/// - consuming active rows, removed rows, and context at conversion boundaries.
///
/// It intentionally does not expose mutable row references, arbitrary
/// active/removed map mutation, or semantic operations such as substitution,
/// partial evaluation, propagation, slack conversion, or capability reduction.
/// Those are root [`crate::Instance`] / [`crate::ParametricInstance`]
/// operations that merely induce the row effects above.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstraintCollection<T: ConstraintType> {
    active: BTreeMap<T::ID, T::Created>,
    removed: BTreeMap<T::ID, (T::Created, RemovedReason)>,
    context: ConstraintContextStore<T::ID>,
}

/// Result of rewriting one active constraint through its owning collection.
pub(crate) enum ActiveConstraintUpdate<C> {
    /// Keep the constraint active under the same ID.
    Active(C),
    /// Move the constraint to the removed map under the same ID.
    Removed {
        constraint: C,
        reason: RemovedReason,
    },
}

impl<T: ConstraintType> Default for ConstraintCollection<T> {
    fn default() -> Self {
        Self {
            active: BTreeMap::new(),
            removed: BTreeMap::new(),
            context: ConstraintContextStore::default(),
        }
    }
}

impl<T: ConstraintType> ConstraintCollection<T> {
    /// Construct a collection from active and removed constraint maps.
    ///
    /// # Errors
    ///
    /// Returns an error if the same constraint ID appears in both `active` and `removed`.
    pub fn new(
        active: BTreeMap<T::ID, T::Created>,
        removed: BTreeMap<T::ID, (T::Created, RemovedReason)>,
    ) -> crate::Result<Self> {
        validate_no_key_overlap(
            &active,
            &removed,
            "active constraints",
            "removed constraints",
        )?;
        Ok(Self {
            active,
            removed,
            context: ConstraintContextStore::default(),
        })
    }

    /// Construct a collection together with its label/provenance sidecar store.
    /// Used by the parse boundary, where sidecars for both active and removed
    /// entries are drained from per-element protobuf messages into one store.
    ///
    /// # Errors
    ///
    /// Returns an error if the same constraint ID appears in both `active` and `removed`.
    pub fn with_context(
        active: BTreeMap<T::ID, T::Created>,
        removed: BTreeMap<T::ID, (T::Created, RemovedReason)>,
        context: ConstraintContextStore<T::ID>,
    ) -> crate::Result<Self> {
        validate_no_key_overlap(
            &active,
            &removed,
            "active constraints",
            "removed constraints",
        )?;
        let owned_ids = active
            .keys()
            .chain(removed.keys())
            .copied()
            .collect::<BTreeSet<_>>();
        validate_context_reference_ids(&context, &owned_ids)?;
        Ok(Self {
            active,
            removed,
            context,
        })
    }

    /// Access the per-constraint label/provenance store.
    pub fn context(&self) -> &ConstraintContextStore<T::ID> {
        &self.context
    }

    /// Validate that every label/provenance ID is owned by this collection.
    pub fn validate_context_ids(&self) -> crate::Result<()> {
        let owned_ids = self
            .active
            .keys()
            .chain(self.removed.keys())
            .copied()
            .collect::<BTreeSet<_>>();
        validate_context_reference_ids(&self.context, &owned_ids)
    }

    /// Access active constraints.
    pub fn active(&self) -> &BTreeMap<T::ID, T::Created> {
        &self.active
    }

    /// Access removed constraints with their removal reasons.
    pub fn removed(&self) -> &BTreeMap<T::ID, (T::Created, RemovedReason)> {
        &self.removed
    }

    /// Return whether `id` belongs to either active or removed constraints.
    pub(crate) fn contains_id(&self, id: T::ID) -> bool {
        self.active.contains_key(&id) || self.removed.contains_key(&id)
    }

    /// Replace the context for an ID owned by this collection.
    ///
    /// The collection checks membership before writing sidecars so callers
    /// cannot create orphan label/provenance entries. `owner_name` is used only
    /// to keep host-level error messages precise.
    pub(crate) fn set_context_for_owner(
        &mut self,
        id: T::ID,
        context: ConstraintContext,
        owner_name: &str,
    ) -> crate::Result<()> {
        if !self.contains_id(id) {
            crate::bail!(
                { ?id },
                "Constraint label/provenance references unknown {owner_name} ID {id:?}",
            );
        }
        self.context.insert(id, context);
        Ok(())
    }

    /// Replace an active constraint payload while preserving row identity and context.
    fn replace_active(&mut self, id: T::ID, constraint: T::Created) -> Option<T::Created> {
        if self.active.contains_key(&id) {
            self.active.insert(id, constraint)
        } else {
            None
        }
    }

    /// Replace a removed constraint payload while preserving its removal reason and context.
    fn replace_removed(&mut self, id: T::ID, constraint: T::Created) -> Option<T::Created> {
        self.removed
            .get_mut(&id)
            .map(|(removed_constraint, _reason)| std::mem::replace(removed_constraint, constraint))
    }

    /// Replace an existing active or removed constraint without changing lifecycle.
    ///
    /// Returns [`None`] when `id` is unknown to this collection. Host-level
    /// callers must validate the payload before calling this method.
    pub(crate) fn replace_preserving_lifecycle(
        &mut self,
        id: T::ID,
        constraint: T::Created,
    ) -> Option<T::Created> {
        if self.active.contains_key(&id) {
            self.replace_active(id, constraint)
        } else {
            self.replace_removed(id, constraint)
        }
    }

    /// Rewrite one active row by value while preserving row identity and context.
    ///
    /// The closure receives an owned clone of the current row and returns the
    /// replacement row. On error this collection is unchanged.
    pub(crate) fn update_active(
        &mut self,
        id: T::ID,
        f: impl FnOnce(T::Created) -> crate::Result<T::Created>,
    ) -> crate::Result<bool> {
        let Some(constraint) = self.active.get(&id).cloned() else {
            return Ok(false);
        };
        let updated = f(constraint)?;
        self.active.insert(id, updated);
        Ok(true)
    }

    /// Rewrite all active entries through collection-owned membership operations.
    ///
    /// The closure receives each active constraint by value and returns whether
    /// it stays active or moves to the removed map under the same ID. The
    /// collection preserves context sidecars and active/removed disjointness.
    /// On error, the active map is restored to its original state and no removed
    /// entries are added by this method.
    pub(crate) fn rewrite_active<E>(
        &mut self,
        mut f: impl FnMut(
            T::ID,
            T::Created,
            &ConstraintContextStore<T::ID>,
        ) -> std::result::Result<ActiveConstraintUpdate<T::Created>, E>,
    ) -> std::result::Result<(), E> {
        let original_active = self.active.clone();
        let active = std::mem::take(&mut self.active);
        let mut next_active = BTreeMap::new();
        let mut next_removed = BTreeMap::new();

        for (id, constraint) in active {
            match f(id, constraint, &self.context) {
                Ok(ActiveConstraintUpdate::Active(constraint)) => {
                    next_active.insert(id, constraint);
                }
                Ok(ActiveConstraintUpdate::Removed { constraint, reason }) => {
                    debug_assert!(!self.removed.contains_key(&id));
                    next_removed.insert(id, (constraint, reason));
                }
                Err(error) => {
                    self.active = original_active;
                    return Err(error);
                }
            }
        }

        self.active = next_active;
        self.removed.extend(next_removed);
        debug_assert!(self.validate_context_ids().is_ok());
        Ok(())
    }

    /// Insert an active constraint along with its context in one step.
    ///
    /// `id` must not already be present in either the active or removed map.
    /// The context is written to the store; empty context fields are stored
    /// sparsely (i.e. omitted) by [`ConstraintContextStore::insert`].
    ///
    /// Crate-internal: external callers use the validating `Instance::add_*`
    /// entry points. This primitive bypasses `validate_required_ids`, so the
    /// caller is responsible for ensuring every `id` in
    /// `constraint.required_ids()` exists in the parent instance's variable
    /// store.
    pub(crate) fn insert_active_with_context(
        &mut self,
        id: T::ID,
        constraint: T::Created,
        context: ConstraintContext,
    ) -> crate::Result<()> {
        if self.active.contains_key(&id) {
            crate::bail!(
                { ?id },
                "Constraint ID {id:?} already exists in the active constraint collection",
            );
        }
        if self.removed.contains_key(&id) {
            crate::bail!(
                { ?id },
                "Constraint ID {id:?} already exists in the removed constraint collection",
            );
        }
        self.active.insert(id, constraint);
        self.context.insert(id, context);
        Ok(())
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

    /// Consume the collection and return active entries, removed entries, and sidecars.
    #[allow(clippy::type_complexity)]
    pub fn into_parts(
        self,
    ) -> (
        BTreeMap<T::ID, T::Created>,
        BTreeMap<T::ID, (T::Created, RemovedReason)>,
        ConstraintContextStore<T::ID>,
    ) {
        (self.active, self.removed, self.context)
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

    /// Restore a removed row after host-owned normalization.
    ///
    /// The closure receives an owned clone of the removed payload, its removal
    /// reason, and this collection's context. On error this collection is
    /// unchanged. This keeps lifecycle movement in the collection while leaving
    /// semantic normalization to the host object.
    pub(crate) fn restore_with(
        &mut self,
        id: T::ID,
        f: impl FnOnce(
            T::Created,
            &RemovedReason,
            &ConstraintContextStore<T::ID>,
        ) -> crate::Result<T::Created>,
    ) -> crate::Result<()> {
        let Some((constraint, reason)) = self.removed.get(&id).cloned() else {
            return Err(crate::error!(
                "Removed constraint with ID {:?} not found",
                id
            ));
        };
        let restored = f(constraint, &reason, &self.context)?;
        self.removed.remove(&id);
        debug_assert!(!self.active.contains_key(&id));
        self.active.insert(id, restored);
        debug_assert!(self.validate_context_ids().is_ok());
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
        EvaluatedCollection::with_context(results, removed_reasons, self.context.clone())
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
        SampledCollection::with_context(results, removed_reasons, self.context.clone())
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
///
/// Carries the source [`ConstraintCollection`]'s label/provenance store so that
/// the Solution layer reads the same canonical sidecars as the originating
/// instance.
///
/// This result table owns only evaluated rows, removed reasons, and context
/// sidecars for one constraint family. It validates that removed-reason and
/// context IDs refer to existing evaluated rows, then remains effectively
/// read-oriented: construction, row/sidecar reads, feasibility queries,
/// removed-state queries, host-owned by-value replacement when required, and
/// consumption at conversion boundaries. Global consistency with evaluated
/// decision-variable rows and named functions belongs to [`crate::Solution`].
#[derive(Debug, Clone, PartialEq)]
pub struct EvaluatedCollection<T: ConstraintType> {
    constraints: BTreeMap<T::ID, T::Evaluated>,
    removed_reasons: BTreeMap<T::ID, RemovedReason>,
    context: ConstraintContextStore<T::ID>,
}

impl<T: ConstraintType> std::ops::Deref for EvaluatedCollection<T> {
    type Target = BTreeMap<T::ID, T::Evaluated>;
    fn deref(&self) -> &Self::Target {
        &self.constraints
    }
}

impl<T: ConstraintType> Default for EvaluatedCollection<T> {
    fn default() -> Self {
        Self {
            constraints: BTreeMap::new(),
            removed_reasons: BTreeMap::new(),
            context: ConstraintContextStore::default(),
        }
    }
}

impl<T: ConstraintType> EvaluatedCollection<T> {
    /// Construct an evaluated collection without label/provenance sidecars.
    ///
    /// # Errors
    ///
    /// Returns an error if `removed_reasons` contains an ID that is not present in
    /// `constraints`.
    pub fn new(
        constraints: BTreeMap<T::ID, T::Evaluated>,
        removed_reasons: BTreeMap<T::ID, RemovedReason>,
    ) -> crate::Result<Self> {
        validate_removed_reasons_reference_entries(&constraints, &removed_reasons)?;
        Ok(Self {
            constraints,
            removed_reasons,
            context: ConstraintContextStore::default(),
        })
    }

    /// Construct an evaluated collection together with its label/provenance store.
    /// Used by [`ConstraintCollection::evaluate`] to thread the source
    /// collection's sidecars through unchanged.
    ///
    /// # Errors
    ///
    /// Returns an error if `removed_reasons` contains an ID that is not present in
    /// `constraints`.
    pub fn with_context(
        constraints: BTreeMap<T::ID, T::Evaluated>,
        removed_reasons: BTreeMap<T::ID, RemovedReason>,
        context: ConstraintContextStore<T::ID>,
    ) -> crate::Result<Self> {
        validate_removed_reasons_reference_entries(&constraints, &removed_reasons)?;
        let owned_ids = constraints.keys().copied().collect::<BTreeSet<_>>();
        validate_context_reference_ids(&context, &owned_ids)?;
        Ok(Self {
            constraints,
            removed_reasons,
            context,
        })
    }

    pub fn inner(&self) -> &BTreeMap<T::ID, T::Evaluated> {
        &self.constraints
    }

    /// Replace an evaluated row while preserving removed-state and context sidecars.
    ///
    /// Returns [`None`] when `id` is unknown to this collection.
    pub(crate) fn replace_evaluated(
        &mut self,
        id: T::ID,
        constraint: T::Evaluated,
    ) -> Option<T::Evaluated> {
        if self.constraints.contains_key(&id) {
            self.constraints.insert(id, constraint)
        } else {
            None
        }
    }

    /// Access the removed reasons map.
    pub fn removed_reasons(&self) -> &BTreeMap<T::ID, RemovedReason> {
        &self.removed_reasons
    }

    /// Access the per-constraint label/provenance store.
    pub fn context(&self) -> &ConstraintContextStore<T::ID> {
        &self.context
    }

    /// Validate that every label/provenance ID is owned by this collection.
    pub fn validate_context_ids(&self) -> crate::Result<()> {
        let owned_ids = self.constraints.keys().copied().collect::<BTreeSet<_>>();
        validate_context_reference_ids(&self.context, &owned_ids)
    }

    /// Consume and return constraints, removed reasons, and sidecars.
    #[allow(clippy::type_complexity)]
    pub fn into_parts(
        self,
    ) -> (
        BTreeMap<T::ID, T::Evaluated>,
        BTreeMap<T::ID, RemovedReason>,
        ConstraintContextStore<T::ID>,
    ) {
        (self.constraints, self.removed_reasons, self.context)
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
///
/// Carries the source [`ConstraintCollection`]'s label/provenance store so that
/// the SampleSet layer reads the same canonical sidecars as the originating
/// instance.
///
/// This result table owns only sampled rows, removed reasons, and context
/// sidecars for one constraint family. It validates that removed-reason and
/// context IDs refer to existing sampled rows, exposes read and feasibility
/// queries, validates sampled-row sample IDs against a host-supplied sample set,
/// validates used decision-variable IDs against a host-supplied variable set,
/// and can be consumed at conversion boundaries. Global sample consistency
/// across tables belongs to [`crate::SampleSet`].
#[derive(Debug, Clone)]
pub struct SampledCollection<T: ConstraintType> {
    constraints: BTreeMap<T::ID, T::Sampled>,
    removed_reasons: BTreeMap<T::ID, RemovedReason>,
    context: ConstraintContextStore<T::ID>,
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
            context: ConstraintContextStore::default(),
        }
    }
}

impl<T: ConstraintType> SampledCollection<T> {
    /// Construct a sampled collection without label/provenance sidecars.
    ///
    /// # Errors
    ///
    /// Returns an error if `removed_reasons` contains an ID that is not present in
    /// `constraints`.
    pub fn new(
        constraints: BTreeMap<T::ID, T::Sampled>,
        removed_reasons: BTreeMap<T::ID, RemovedReason>,
    ) -> crate::Result<Self> {
        validate_removed_reasons_reference_entries(&constraints, &removed_reasons)?;
        Ok(Self {
            constraints,
            removed_reasons,
            context: ConstraintContextStore::default(),
        })
    }

    /// Construct a sampled collection together with its label/provenance store.
    /// Used by [`ConstraintCollection::evaluate_samples`] to thread the
    /// source collection's sidecars through unchanged.
    ///
    /// # Errors
    ///
    /// Returns an error if `removed_reasons` contains an ID that is not present in
    /// `constraints`.
    pub fn with_context(
        constraints: BTreeMap<T::ID, T::Sampled>,
        removed_reasons: BTreeMap<T::ID, RemovedReason>,
        context: ConstraintContextStore<T::ID>,
    ) -> crate::Result<Self> {
        validate_removed_reasons_reference_entries(&constraints, &removed_reasons)?;
        let owned_ids = constraints.keys().copied().collect::<BTreeSet<_>>();
        validate_context_reference_ids(&context, &owned_ids)?;
        Ok(Self {
            constraints,
            removed_reasons,
            context,
        })
    }

    pub fn inner(&self) -> &BTreeMap<T::ID, T::Sampled> {
        &self.constraints
    }

    /// Validate that every sampled constraint in this collection carries the
    /// same sample IDs as `expected` across all of its per-sample side maps.
    pub fn validate_sample_ids(
        &self,
        expected: &SampleIDSet,
    ) -> std::result::Result<(), SampleIDSet> {
        for constraint in self.constraints.values() {
            constraint.validate_sample_ids(expected)?;
        }
        Ok(())
    }

    /// Validate that every sampled constraint only references known decision variables.
    pub fn validate_used_decision_variable_ids(
        &self,
        decision_variable_ids: &BTreeSet<crate::VariableID>,
    ) -> std::result::Result<(), (T::ID, crate::VariableID)> {
        for (constraint_id, constraint) in &self.constraints {
            for var_id in constraint.used_decision_variable_ids() {
                if !decision_variable_ids.contains(var_id) {
                    return Err((*constraint_id, *var_id));
                }
            }
        }
        Ok(())
    }

    /// Access the removed reasons map.
    pub fn removed_reasons(&self) -> &BTreeMap<T::ID, RemovedReason> {
        &self.removed_reasons
    }

    /// Access the per-constraint label/provenance store.
    pub fn context(&self) -> &ConstraintContextStore<T::ID> {
        &self.context
    }

    /// Validate that every label/provenance ID is owned by this collection.
    pub fn validate_context_ids(&self) -> crate::Result<()> {
        let owned_ids = self.constraints.keys().copied().collect::<BTreeSet<_>>();
        validate_context_reference_ids(&self.context, &owned_ids)
    }

    /// Consume and return constraints, removed reasons, and sidecars.
    #[allow(clippy::type_complexity)]
    pub fn into_parts(
        self,
    ) -> (
        BTreeMap<T::ID, T::Sampled>,
        BTreeMap<T::ID, RemovedReason>,
        ConstraintContextStore<T::ID>,
    ) {
        (self.constraints, self.removed_reasons, self.context)
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
    use crate::{coeff, constraint::ConstraintID, linear, Equality, Function, ModelingLabel};

    fn removed_reason() -> RemovedReason {
        RemovedReason {
            reason: "test".to_string(),
            parameters: Default::default(),
        }
    }

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

        let collection = ConstraintCollection::<Constraint>::new(active, BTreeMap::new()).unwrap();

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

        let collection = ConstraintCollection::<Constraint>::new(active, BTreeMap::new()).unwrap();
        assert_eq!(collection.active().len(), 1);
        assert_eq!(collection.removed().len(), 0);
    }

    #[test]
    fn collection_rejects_active_removed_overlap() {
        let id = ConstraintID::from(1);
        let active = BTreeMap::from([(id, Constraint::equal_to_zero(Function::Zero))]);
        let removed = BTreeMap::from([(
            id,
            (Constraint::equal_to_zero(Function::Zero), removed_reason()),
        )]);

        let err = ConstraintCollection::<Constraint>::new(active, removed).unwrap_err();
        assert!(err
            .to_string()
            .contains("appears in both active constraints and removed constraints"));
    }

    #[test]
    fn collection_rejects_orphan_context_id() {
        let id = ConstraintID::from(1);
        let orphan_id = ConstraintID::from(99);
        let active = BTreeMap::from([(id, Constraint::equal_to_zero(Function::Zero))]);
        let mut context = ConstraintContextStore::default();
        context.set_name(orphan_id, "orphan");

        let err =
            ConstraintCollection::<Constraint>::with_context(active, BTreeMap::new(), context)
                .unwrap_err();

        assert!(
            err.to_string().contains("unknown constraint ID")
                && err.to_string().contains("ConstraintID(99)"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn set_context_for_owner_rejects_unknown_id_without_orphan() {
        let id = ConstraintID::from(1);
        let orphan_id = ConstraintID::from(99);
        let active = BTreeMap::from([(id, Constraint::equal_to_zero(Function::Zero))]);
        let mut collection =
            ConstraintCollection::<Constraint>::new(active, BTreeMap::new()).unwrap();

        let context = ConstraintContext {
            label: ModelingLabel {
                name: Some("orphan".to_string()),
                ..Default::default()
            },
            provenance: vec![],
        };
        let err = collection
            .set_context_for_owner(orphan_id, context, "constraint")
            .unwrap_err();

        assert!(
            err.to_string().contains("unknown constraint ID")
                && err.to_string().contains("ConstraintID(99)"),
            "unexpected error: {err}"
        );
        assert!(!collection.context().contains(orphan_id));
        collection.validate_context_ids().unwrap();
    }

    #[test]
    fn rewrite_active_moves_removed_entries_without_orphaning_context() {
        let removed_id = ConstraintID::from(1);
        let active_id = ConstraintID::from(2);
        let active = BTreeMap::from([
            (removed_id, Constraint::equal_to_zero(Function::Zero)),
            (
                active_id,
                Constraint::equal_to_zero(Function::from(linear!(1))),
            ),
        ]);
        let mut context = ConstraintContextStore::default();
        context.set_name(removed_id, "original");
        let mut collection =
            ConstraintCollection::<Constraint>::with_context(active, BTreeMap::new(), context)
                .unwrap();

        let result: std::result::Result<(), std::convert::Infallible> =
            collection.rewrite_active(|id, constraint, _context| {
                if id == removed_id {
                    Ok(ActiveConstraintUpdate::Removed {
                        constraint,
                        reason: removed_reason(),
                    })
                } else {
                    Ok(ActiveConstraintUpdate::Active(constraint))
                }
            });
        result.unwrap();

        assert!(!collection.active().contains_key(&removed_id));
        assert!(collection.active().contains_key(&active_id));
        assert!(collection.removed().contains_key(&removed_id));
        assert_eq!(collection.context().name(removed_id), Some("original"));
        collection.validate_context_ids().unwrap();
    }

    #[test]
    fn update_active_keeps_collection_unchanged_on_error() {
        let id = ConstraintID::from(1);
        let original = Constraint::equal_to_zero(Function::Zero);
        let mut collection = ConstraintCollection::<Constraint>::new(
            BTreeMap::from([(id, original.clone())]),
            BTreeMap::new(),
        )
        .unwrap();

        let err = collection
            .update_active(id, |mut constraint| {
                constraint.equality = Equality::LessThanOrEqualToZero;
                Err(crate::error!("planned failure"))
            })
            .unwrap_err();

        assert!(err.to_string().contains("planned failure"));
        assert_eq!(collection.active().get(&id), Some(&original));
        assert!(collection.removed().is_empty());
        collection.validate_context_ids().unwrap();
    }

    #[test]
    fn restore_with_normalizes_removed_row_atomically() {
        let id = ConstraintID::from(1);
        let removed = Constraint::less_than_or_equal_to_zero(Function::Zero);
        let mut context = ConstraintContextStore::default();
        context.set_name(id, "restored");
        let mut collection = ConstraintCollection::<Constraint>::with_context(
            BTreeMap::new(),
            BTreeMap::from([(id, (removed.clone(), removed_reason()))]),
            context,
        )
        .unwrap();

        let err = collection
            .restore_with(id, |_constraint, _reason, _context| {
                Err(crate::error!("planned failure"))
            })
            .unwrap_err();
        assert!(err.to_string().contains("planned failure"));
        assert!(!collection.active().contains_key(&id));
        assert_eq!(
            collection.removed().get(&id).map(|(c, _)| c),
            Some(&removed)
        );
        assert_eq!(collection.context().name(id), Some("restored"));

        collection
            .restore_with(id, |mut constraint, reason, context| {
                assert_eq!(reason.reason, "test");
                assert_eq!(context.name(id), Some("restored"));
                constraint.equality = Equality::EqualToZero;
                Ok(constraint)
            })
            .unwrap();

        assert_eq!(
            collection.active().get(&id).map(|c| c.equality),
            Some(Equality::EqualToZero)
        );
        assert!(!collection.removed().contains_key(&id));
        assert_eq!(collection.context().name(id), Some("restored"));
        collection.validate_context_ids().unwrap();
    }

    #[test]
    fn insert_active_with_context_rejects_duplicate_ids() {
        let id = ConstraintID::from(1);
        let mut collection = ConstraintCollection::<Constraint>::new(
            BTreeMap::from([(id, Constraint::equal_to_zero(Function::Zero))]),
            BTreeMap::new(),
        )
        .unwrap();

        let err = collection
            .insert_active_with_context(
                id,
                Constraint::equal_to_zero(Function::Zero),
                ConstraintContext::default(),
            )
            .unwrap_err();
        assert!(err.to_string().contains("already exists in the active"));

        let removed_id = ConstraintID::from(2);
        let mut collection = ConstraintCollection::<Constraint>::new(
            BTreeMap::new(),
            BTreeMap::from([(
                removed_id,
                (Constraint::equal_to_zero(Function::Zero), removed_reason()),
            )]),
        )
        .unwrap();

        let err = collection
            .insert_active_with_context(
                removed_id,
                Constraint::equal_to_zero(Function::Zero),
                ConstraintContext::default(),
            )
            .unwrap_err();
        assert!(err.to_string().contains("already exists in the removed"));
    }

    #[test]
    fn evaluated_collection_rejects_removed_reason_without_constraint() {
        let constraints: BTreeMap<ConstraintID, EvaluatedConstraint> = BTreeMap::new();
        let removed_reasons = BTreeMap::from([(ConstraintID::from(1), removed_reason())]);

        let err = EvaluatedCollection::<Constraint>::new(constraints, removed_reasons).unwrap_err();
        assert!(err
            .to_string()
            .contains("Removed reason references unknown constraint ID"));
    }

    #[test]
    fn sampled_collection_rejects_removed_reason_without_constraint() {
        let constraints: BTreeMap<ConstraintID, SampledConstraint> = BTreeMap::new();
        let removed_reasons = BTreeMap::from([(ConstraintID::from(1), removed_reason())]);

        let err = SampledCollection::<Constraint>::new(constraints, removed_reasons).unwrap_err();
        assert!(err
            .to_string()
            .contains("Removed reason references unknown constraint ID"));
    }
}
