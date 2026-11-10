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
//! 4. Implement `Evaluate` for `NewConstraint<Created>`,
//!    [`EvaluatedConstraintData`] and [`SampledConstraintData`]. Every family must
//!    provide a nonnegative scalar violation. Feasibility is derived by the
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

use crate::Result;
use crate::{
    constraint::{
        ConstraintContext, ConstraintContextStore, ConstraintID, EvaluatedConstraint,
        InactiveConstraintMetadata, SampledConstraint,
    },
    v1, ATol, Constraint, Evaluate, Parse, ParseError, SampleID, SampleIDSet, VariableIDSet,
};
use std::sync::LazyLock;

static EMPTY_VARIABLE_ID_SET: LazyLock<VariableIDSet> = LazyLock::new(VariableIDSet::default);
use std::collections::{btree_map::Entry, BTreeMap, BTreeSet};

#[cfg(test)]
mod violation_tests;

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

fn validate_inactive_metadata_reference_entries<ID, V>(
    constraints: &BTreeMap<ID, V>,
    inactive_metadata: &BTreeMap<ID, InactiveConstraintMetadata>,
) -> crate::Result<()>
where
    ID: IDType,
{
    if let Some(id) = inactive_metadata
        .keys()
        .find(|id| !constraints.contains_key(id))
    {
        crate::bail!({ ?id }, "Inactive metadata references unknown constraint ID {id:?}");
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
///
/// ```compile_fail,E0119
/// use ommx::{ATol, ConstraintID, EvaluatedConstraintData, EvaluatedConstraintBehavior};
/// struct NewConstraint;
/// impl EvaluatedConstraintData for NewConstraint {
///     type ID = ConstraintID;
///     fn violation(&self) -> f64 { 0.0 }
/// }
/// // This conflicts with the common blanket implementation.
/// impl EvaluatedConstraintBehavior for NewConstraint {
///     fn is_feasible(&self, _: ATol) -> bool { false }
/// }
/// ```
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

/// Storage effect produced by a by-value rewrite of an active constraint row.
///
/// The enclosing domain owner decides which effect applies. The collection only
/// commits the corresponding lifecycle change while retaining the row ID and
/// its context sidecars.
pub(crate) enum ActiveRowRewrite<T> {
    Active(T),
    Inactive(T, InactiveConstraintMetadata),
}

// ===== Evaluation data supplied by regular constraints =====

impl EvaluatedConstraintData for EvaluatedConstraint {
    type ID = ConstraintID;
    fn violation(&self) -> f64 {
        self.equality.violation(self.stage.evaluated_value)
    }

    fn used_decision_variable_ids(&self) -> &VariableIDSet {
        &self.stage.used_decision_variable_ids
    }
}

impl SampledConstraintData for SampledConstraint {
    type ID = ConstraintID;
    type Evaluated = EvaluatedConstraint;

    fn violation_for(&self, sample_id: SampleID) -> Option<f64> {
        self.stage
            .evaluated_values
            .get(sample_id)
            .map(|value| self.equality.violation(*value))
    }

    fn validate_sample_ids(&self, expected: &SampleIDSet) -> std::result::Result<(), SampleIDSet> {
        if !self.stage.evaluated_values.has_same_ids(expected) {
            return Err(self.stage.evaluated_values.ids());
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

        Some(crate::Constraint {
            equality: self.equality,
            stage: EvaluatedData {
                evaluated_value,
                dual_variable,
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

/// A collection of active and inactive constraints of the same type.
///
/// Inactive constraints are stored as `(T::Created, InactiveConstraintMetadata)` pairs.
/// The `InactiveConstraintMetadata` is deactivation state, not part of the constraint itself.
///
/// Per-constraint modeling labels and transformation provenance are held by
/// [`Self::context`] in Struct-of-Arrays form keyed by `T::ID`. The store
/// rides through to [`EvaluatedCollection`] / [`SampledCollection`] on
/// evaluation, so the modeling, Solution, and SampleSet layers all read from
/// one canonical sidecar source per collection.
///
/// This collection owns the table-level invariants for one constraint family:
///
/// - active and inactive IDs are disjoint;
/// - inactive metadata exists only for inactive constraints;
/// - every label/provenance sidecar ID belongs to either an active or inactive
///   constraint in this collection.
///
/// Host objects such as [`crate::Instance`] and [`crate::ParametricInstance`]
/// still own cross-table semantic invariants, including referenced
/// decision-variable IDs, numeric constraint-ID uniqueness across all families,
/// and special-constraint structural requirements.
///
/// # Family-local operations
///
/// Mathematically, this is one constraint-family component
/// `C_tau = Active_tau + Inactive_tau + Context_tau` of an enclosing instance.
/// It supports only family-local row effects:
///
/// - construction from active rows, inactive rows, and context;
/// - read access to active rows, inactive rows, and context;
/// - fresh active-row insertion together with context;
/// - lifecycle-preserving row replacement after host validation;
/// - by-value active-row rewrites that either keep rows active or move them to
///   inactive with a host-supplied reason;
/// - active-to-inactive lifecycle movement;
/// - activation through a host-supplied normalizer;
/// - context updates for IDs owned by this collection;
/// - consuming active rows, inactive rows, and context at conversion boundaries.
///
/// It intentionally does not expose mutable row references, arbitrary
/// active/inactive map mutation, or semantic operations such as substitution,
/// partial evaluation, propagation, slack conversion, preparation, or
/// special-constraint lowering. When implementing a model transformation, use
/// the enclosing [`crate::Instance`] / [`crate::ParametricInstance`] operation
/// to preserve cross-table invariants; the collection handles only the row
/// effects above.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstraintCollection<T: ConstraintType> {
    active: BTreeMap<T::ID, T::Created>,
    inactive: BTreeMap<T::ID, (T::Created, InactiveConstraintMetadata)>,
    context: ConstraintContextStore<T::ID>,
}

impl<T: ConstraintType> Default for ConstraintCollection<T> {
    fn default() -> Self {
        Self {
            active: BTreeMap::new(),
            inactive: BTreeMap::new(),
            context: ConstraintContextStore::default(),
        }
    }
}

impl<T: ConstraintType> ConstraintCollection<T> {
    /// Construct a collection from active and inactive constraint maps.
    ///
    /// # Errors
    ///
    /// Returns an error if the same constraint ID appears in both `active` and `inactive`.
    pub fn new(
        active: BTreeMap<T::ID, T::Created>,
        inactive: BTreeMap<T::ID, (T::Created, InactiveConstraintMetadata)>,
    ) -> crate::Result<Self> {
        validate_no_key_overlap(
            &active,
            &inactive,
            "active constraints",
            "inactive constraints",
        )?;
        Ok(Self {
            active,
            inactive,
            context: ConstraintContextStore::default(),
        })
    }

    /// Construct a collection together with its label/provenance sidecar store.
    /// Used by the parse boundary, where sidecars for both active and inactive
    /// entries are drained from per-element protobuf messages into one store.
    ///
    /// # Errors
    ///
    /// Returns an error if the same constraint ID appears in both `active` and `inactive`.
    pub fn with_context(
        active: BTreeMap<T::ID, T::Created>,
        inactive: BTreeMap<T::ID, (T::Created, InactiveConstraintMetadata)>,
        context: ConstraintContextStore<T::ID>,
    ) -> crate::Result<Self> {
        validate_no_key_overlap(
            &active,
            &inactive,
            "active constraints",
            "inactive constraints",
        )?;
        let owned_ids = active
            .keys()
            .chain(inactive.keys())
            .copied()
            .collect::<BTreeSet<_>>();
        validate_context_reference_ids(&context, &owned_ids)?;
        Ok(Self {
            active,
            inactive,
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
            .chain(self.inactive.keys())
            .copied()
            .collect::<BTreeSet<_>>();
        validate_context_reference_ids(&self.context, &owned_ids)
    }

    /// Access active constraints.
    pub fn active(&self) -> &BTreeMap<T::ID, T::Created> {
        &self.active
    }

    /// Access inactive constraints with their inactive metadata.
    pub fn inactive(&self) -> &BTreeMap<T::ID, (T::Created, InactiveConstraintMetadata)> {
        &self.inactive
    }

    /// Look up a constraint's participation independently of its family.
    pub fn activeness(&self, id: &T::ID) -> Option<crate::ConstraintActiveness> {
        if self.active.contains_key(id) {
            Some(crate::ConstraintActiveness::Active)
        } else {
            self.inactive
                .get(id)
                .map(|(_, metadata)| metadata.activeness())
        }
    }

    /// All retained rows, without their membership metadata.
    pub fn iter(&self) -> impl Iterator<Item = (&T::ID, &T::Created)> {
        self.active
            .iter()
            .chain(self.inactive.iter().map(|(id, (row, _))| (id, row)))
    }

    /// IDs of all retained constraints.
    pub fn keys(&self) -> impl Iterator<Item = &T::ID> {
        self.iter().map(|(id, _)| id)
    }

    /// All retained constraint payloads.
    pub fn values(&self) -> impl Iterator<Item = &T::Created> {
        self.iter().map(|(_, row)| row)
    }

    /// Look up a retained row regardless of its activeness.
    pub fn get(&self, id: &T::ID) -> Option<&T::Created> {
        self.active
            .get(id)
            .or_else(|| self.inactive.get(id).map(|(row, _)| row))
    }

    /// Number of active and inactive rows.
    pub fn len(&self) -> usize {
        self.active.len() + self.inactive.len()
    }

    /// Whether this collection contains no retained rows.
    pub fn is_empty(&self) -> bool {
        self.active.is_empty() && self.inactive.is_empty()
    }

    /// Change default feasibility participation for an existing inactive row.
    pub(crate) fn set_skip_feasibility_check(
        &mut self,
        id: T::ID,
        skip: bool,
    ) -> crate::Result<()> {
        let (_, metadata) = self
            .inactive
            .get_mut(&id)
            .ok_or_else(|| crate::error!("Inactive constraint with ID {id:?} not found"))?;
        metadata.skip_feasibility_check = skip;
        Ok(())
    }

    /// Return whether `id` belongs to either active or inactive constraints.
    fn contains_id(&self, id: T::ID) -> bool {
        self.active.contains_key(&id) || self.inactive.contains_key(&id)
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
        match self.active.entry(id) {
            Entry::Occupied(mut entry) => Some(entry.insert(constraint)),
            Entry::Vacant(_) => None,
        }
    }

    /// Replace an inactive constraint payload while preserving its inactive metadata and context.
    fn replace_inactive(&mut self, id: T::ID, constraint: T::Created) -> Option<T::Created> {
        self.inactive
            .get_mut(&id)
            .map(|(inactive_constraint, _reason)| {
                std::mem::replace(inactive_constraint, constraint)
            })
    }

    /// Replace an existing active or inactive constraint without changing lifecycle.
    ///
    /// Returns [`None`] when `id` is unknown to this collection. Host-level
    /// callers must validate the payload before calling this method.
    pub(crate) fn replace_row_preserving_lifecycle(
        &mut self,
        id: T::ID,
        constraint: T::Created,
    ) -> Option<T::Created> {
        if self.active.contains_key(&id) {
            self.replace_active(id, constraint)
        } else {
            self.replace_inactive(id, constraint)
        }
    }

    /// Replace active and inactive row payloads without changing lifecycle or context.
    ///
    /// The parent object owns semantic transformations such as parameter
    /// substitution. This collection only validates and commits the storage
    /// effect: active replacements must refer to active rows, inactive
    /// replacements must refer to inactive rows, and inactive metadata plus
    /// context sidecars are preserved.
    pub(crate) fn replace_rows_preserving_lifecycle(
        &mut self,
        active_replacements: BTreeMap<T::ID, T::Created>,
        inactive_replacements: BTreeMap<T::ID, T::Created>,
    ) -> crate::Result<()> {
        validate_no_key_overlap(
            &active_replacements,
            &inactive_replacements,
            "active row replacements",
            "inactive row replacements",
        )?;
        for id in active_replacements.keys() {
            if !self.active.contains_key(id) {
                crate::bail!({ ?id }, "Active constraint with ID {id:?} not found");
            }
        }
        for id in inactive_replacements.keys() {
            if !self.inactive.contains_key(id) {
                crate::bail!({ ?id }, "Inactive constraint with ID {id:?} not found");
            }
        }

        for (id, constraint) in active_replacements {
            self.active.insert(id, constraint);
        }
        for (id, constraint) in inactive_replacements {
            let (inactive_constraint, _reason) = self
                .inactive
                .get_mut(&id)
                .expect("inactive row was validated before replacement");
            *inactive_constraint = constraint;
        }
        debug_assert!(self.validate_context_ids().is_ok());
        Ok(())
    }

    /// Apply precomputed effects on active rows.
    ///
    /// The parent object owns semantic transformations such as substitution,
    /// propagation, or penalty conversion. This collection only validates and
    /// commits the row-storage effects those operations induce: replacing
    /// active rows in place, and moving active rows to the inactive map with a
    /// reason. Row identity and context sidecars are preserved.
    pub(crate) fn replace_and_remove_active_rows(
        &mut self,
        replacements: BTreeMap<T::ID, T::Created>,
        deactivations: BTreeMap<T::ID, (T::Created, InactiveConstraintMetadata)>,
    ) -> crate::Result<()> {
        if let Some(id) = replacements
            .keys()
            .find(|id| deactivations.contains_key(id))
        {
            crate::bail!(
                { ?id },
                "Constraint ID {id:?} appears in both active row replacements and deactivations",
            );
        }
        for id in replacements.keys().chain(deactivations.keys()) {
            if !self.active.contains_key(id) {
                crate::bail!({ ?id }, "Active constraint with ID {id:?} not found");
            }
        }
        for (id, constraint) in replacements {
            self.active.insert(id, constraint);
        }
        for (id, (constraint, reason)) in deactivations {
            self.active
                .remove(&id)
                .expect("active row was validated before deactivation");
            self.inactive.insert(id, (constraint, reason));
        }
        debug_assert!(self.validate_context_ids().is_ok());
        Ok(())
    }

    /// Consume and rewrite every active row without cloning its payload.
    ///
    /// The caller owns the semantic rewrite and chooses whether each row stays
    /// active or moves to the inactive set. This collection-owned operation keeps
    /// active/inactive IDs disjoint and carries the existing context sidecars
    /// through unchanged.
    ///
    /// This is intended for consuming root-object operations, or for operations
    /// performed on a root-level rollback copy. Since the closure consumes each
    /// row, an error consumes this collection rather than returning a partially
    /// rewritten value.
    pub(crate) fn rewrite_active_rows_by_value(
        self,
        mut rewrite: impl FnMut(T::ID, T::Created) -> crate::Result<ActiveRowRewrite<T::Created>>,
    ) -> crate::Result<Self> {
        let Self {
            active,
            mut inactive,
            context,
        } = self;
        let mut rewritten_active = BTreeMap::new();

        for (id, row) in active {
            match rewrite(id, row)? {
                ActiveRowRewrite::Active(row) => {
                    rewritten_active.insert(id, row);
                }
                ActiveRowRewrite::Inactive(row, reason) => {
                    let previous = inactive.insert(id, (row, reason));
                    debug_assert!(previous.is_none(), "active and inactive IDs are disjoint");
                }
            }
        }

        let rewritten = Self {
            active: rewritten_active,
            inactive,
            context,
        };
        debug_assert!(rewritten.validate_context_ids().is_ok());
        Ok(rewritten)
    }

    /// Replace active rows in place while preserving row identity and context.
    pub(crate) fn replace_active_rows(
        &mut self,
        replacements: BTreeMap<T::ID, T::Created>,
    ) -> crate::Result<()> {
        self.replace_and_remove_active_rows(replacements, BTreeMap::new())
    }

    /// Replace one active row in place while preserving row identity and context.
    pub(crate) fn replace_active_row(
        &mut self,
        id: T::ID,
        constraint: T::Created,
    ) -> crate::Result<()> {
        self.replace_active_rows(BTreeMap::from([(id, constraint)]))
    }

    /// Move existing active payloads to the inactive set with host-planned reasons.
    ///
    /// Crate-internal root operations use this storage effect after completing
    /// all fallible semantic work, so unchanged payloads are moved rather than
    /// cloned into a replacement map.
    pub(crate) fn move_active_rows_to_inactive_with_reasons(
        &mut self,
        reasons: BTreeMap<T::ID, InactiveConstraintMetadata>,
    ) -> crate::Result<()> {
        for id in reasons.keys() {
            if !self.active.contains_key(id) {
                crate::bail!({ ?id }, "Active constraint with ID {id:?} not found");
            }
        }

        for (id, reason) in reasons {
            let constraint = self
                .active
                .remove(&id)
                .expect("active row was validated before deactivation");
            self.inactive.insert(id, (constraint, reason));
        }
        debug_assert!(self.validate_context_ids().is_ok());
        Ok(())
    }

    /// Insert an active constraint along with its context in one step.
    ///
    /// `id` must not already be present in either the active or inactive map.
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
        if self.inactive.contains_key(&id) {
            crate::bail!(
                { ?id },
                "Constraint ID {id:?} already exists in the inactive constraint collection",
            );
        }
        self.active.insert(id, constraint);
        self.context.insert(id, context);
        Ok(())
    }

    fn maximum_owned_id(&self) -> Option<u64> {
        let max_active = self.active.keys().last().copied().map(Into::into);
        let max_inactive = self.inactive.keys().last().copied().map(Into::into);
        match (max_active, max_inactive) {
            (None, None) => None,
            (Some(id), None) | (None, Some(id)) => Some(id),
            (Some(active), Some(inactive)) => Some(active.max(inactive)),
        }
    }

    /// Return an ID that is not used by any active or inactive constraint in this collection.
    ///
    /// Returns `0` when the collection is empty, otherwise `max(existing id) + 1`.
    /// This is only a collection-local query. Model insertion must use the
    /// enclosing instance's allocator, which considers every constraint family.
    ///
    /// # Panics
    ///
    /// Panics if the maximum existing ID is `u64::MAX`, so no larger automatic
    /// ID can be assigned. Gaps below the maximum are not reused.
    pub fn unused_id(&self) -> T::ID {
        let next = self
            .maximum_owned_id()
            .map(|id| {
                id.checked_add(1)
                    .expect("no constraint ID larger than the current maximum remains")
            })
            .unwrap_or(0);
        T::ID::from(next)
    }

    /// Move an active constraint to the inactive set with its check policy and reason.
    pub fn deactivate(
        &mut self,
        id: T::ID,
        metadata: InactiveConstraintMetadata,
    ) -> crate::Result<()> {
        let c = self
            .active
            .remove(&id)
            .ok_or_else(|| crate::error!("Constraint with ID {:?} not found", id))?;
        self.inactive.insert(id, (c, metadata));
        Ok(())
    }

    /// Activate an inactive row using a host-normalized payload.
    ///
    /// The parent object owns semantic normalization before activation because it
    /// may depend on cross-table facts such as fixed variables or decision
    /// variable dependencies. This collection only moves the row between
    /// lifecycle maps while preserving context sidecars.
    pub(crate) fn activate_row(
        &mut self,
        id: T::ID,
        constraint: T::Created,
    ) -> crate::Result<InactiveConstraintMetadata> {
        let Some((_inactive, reason)) = self.inactive.remove(&id) else {
            return Err(crate::error!(
                "Inactive constraint with ID {:?} not found",
                id
            ));
        };
        debug_assert!(!self.active.contains_key(&id));
        self.active.insert(id, constraint);
        debug_assert!(self.validate_context_ids().is_ok());
        Ok(reason)
    }

    /// Decision-variable IDs referenced by this collection's active constraints.
    ///
    /// Inactive constraints do not contribute, regardless of their feasibility
    /// check policy. Use [`Evaluate::required_ids`] for the IDs needed to evaluate
    /// the entire collection, including inactive constraints.
    pub fn active_decision_variable_ids(&self) -> VariableIDSet {
        let mut ids = VariableIDSet::default();
        for constraint in self.active.values() {
            ids.extend(constraint.required_ids());
        }
        ids
    }
}

impl TryFrom<ConstraintCollection<Constraint>>
    for (Vec<v1::Constraint>, Vec<v1::RemovedConstraint>)
{
    type Error = crate::Error;

    fn try_from(value: ConstraintCollection<Constraint>) -> crate::Result<Self> {
        crate::v1_io::validate_inactive_metadata(
            value.inactive().values().map(|(_, metadata)| metadata),
        )?;
        let ConstraintCollection {
            active,
            inactive,
            mut context,
        } = value;
        let active = active
            .into_iter()
            .map(|(id, constraint)| constraint_to_v1(id, constraint, context.remove(id)))
            .collect();
        let inactive = inactive
            .into_iter()
            .map(|(id, (constraint, reason))| {
                inactive_constraint_to_v1(id, constraint, context.remove(id), reason)
            })
            .collect();
        Ok((active, inactive))
    }
}

fn constraint_to_v1(
    id: ConstraintID,
    value: Constraint,
    context: ConstraintContext,
) -> v1::Constraint {
    let label = context.label;
    v1::Constraint {
        id: id.into_inner(),
        equality: value.equality.into(),
        function: Some(value.stage.function.into()),
        name: label.name,
        subscripts: label.subscripts,
        parameters: label.parameters.into_iter().collect(),
        description: label.description,
    }
}

fn inactive_constraint_to_v1(
    id: ConstraintID,
    constraint: Constraint,
    context: ConstraintContext,
    inactive_metadata: InactiveConstraintMetadata,
) -> v1::RemovedConstraint {
    v1::RemovedConstraint {
        constraint: Some(constraint_to_v1(id, constraint, context)),
        removed_reason: inactive_metadata.reason,
        removed_reason_parameters: inactive_metadata.parameters.into_iter().collect(),
    }
}

macro_rules! impl_v2_created_collection {
    ($source:ty => $target:ty) => {
        impl From<ConstraintCollection<$source>> for $target {
            fn from(value: ConstraintCollection<$source>) -> Self {
                let ConstraintCollection {
                    active,
                    inactive,
                    context,
                } = value;
                let (inactive, inactive_metadata) = inactive_entries_to_v2_maps(inactive);
                Self {
                    active: entries_to_v2_map(active),
                    inactive,
                    inactive_metadata,
                    contexts: constraint_context_store_to_v2_map(&context),
                }
            }
        }
    };
}

impl_v2_created_collection!(Constraint => crate::v2::RegularConstraintCollection);
impl_v2_created_collection!(crate::IndicatorConstraint => crate::v2::IndicatorConstraintCollection);
impl_v2_created_collection!(crate::OneHotConstraint => crate::v2::OneHotConstraintCollection);
impl_v2_created_collection!(crate::Sos1Constraint => crate::v2::Sos1ConstraintCollection);

macro_rules! impl_v2_evaluated_collection {
    ($source:ty => $target:ident) => {
        impl EvaluatedCollection<$source> {
            /// Solution/SampleSet supplies the shared wire feasibility tolerance.
            pub(crate) fn into_v2(self, atol: ATol) -> crate::v2::$target {
                let value = self;
                let EvaluatedCollection {
                    constraints,
                    inactive_metadata,
                    context,
                } = value;
                crate::v2::$target {
                    entries: constraints
                        .into_iter()
                        .map(|(id, row)| (id.into(), row.into_v2(atol)))
                        .collect(),
                    inactive_metadata: inactive_metadata_to_v2_map(inactive_metadata),
                    contexts: constraint_context_store_to_v2_map(&context),
                }
            }
        }
    };
}

impl_v2_evaluated_collection!(Constraint => EvaluatedRegularConstraintCollection);
impl_v2_evaluated_collection!(crate::IndicatorConstraint => EvaluatedIndicatorConstraintCollection);
impl_v2_evaluated_collection!(crate::OneHotConstraint => EvaluatedOneHotConstraintCollection);
impl_v2_evaluated_collection!(crate::Sos1Constraint => EvaluatedSos1ConstraintCollection);

macro_rules! impl_v2_sampled_collection {
    ($source:ty => $target:ident) => {
        impl SampledCollection<$source> {
            /// Solution/SampleSet supplies the shared wire feasibility tolerance.
            pub(crate) fn into_v2(self, atol: ATol) -> crate::v2::$target {
                let value = self;
                let SampledCollection {
                    constraints,
                    inactive_metadata,
                    context,
                } = value;
                crate::v2::$target {
                    entries: constraints
                        .into_iter()
                        .map(|(id, row)| (id.into(), row.into_v2(atol)))
                        .collect(),
                    inactive_metadata: inactive_metadata_to_v2_map(inactive_metadata),
                    contexts: constraint_context_store_to_v2_map(&context),
                }
            }
        }
    };
}

impl_v2_sampled_collection!(Constraint => SampledRegularConstraintCollection);
impl_v2_sampled_collection!(crate::IndicatorConstraint => SampledIndicatorConstraintCollection);
impl_v2_sampled_collection!(crate::OneHotConstraint => SampledOneHotConstraintCollection);
impl_v2_sampled_collection!(crate::Sos1Constraint => SampledSos1ConstraintCollection);

fn constraint_context_store_to_v2_map<ID: IDType>(
    store: &ConstraintContextStore<ID>,
) -> BTreeMap<u64, crate::v2::ConstraintContext> {
    store
        .ids()
        .into_iter()
        .map(|id| (id.into(), store.collect_for(id).into()))
        .collect()
}

fn constraint_context_store_from_v2_map<ID: IDType>(
    contexts: BTreeMap<u64, crate::v2::ConstraintContext>,
    message: &'static str,
    field: &'static str,
) -> std::result::Result<ConstraintContextStore<ID>, ParseError> {
    let mut store = ConstraintContextStore::default();
    for (id, context) in contexts {
        store.insert(ID::from(id), context.parse_as(&(), message, field)?);
    }
    Ok(store)
}

macro_rules! impl_parse_v2_created_collection {
    ($source:ty => $target:ty) => {
        impl Parse for $target {
            type Output = ConstraintCollection<$source>;
            type Context = ();

            fn parse(self, _: &Self::Context) -> std::result::Result<Self::Output, ParseError> {
                let message = stringify!($target);
                let active = parse_v2_constraint_entries(self.active, message, "active")?;
                let inactive = parse_v2_inactive_constraint_entries(
                    self.inactive,
                    self.inactive_metadata,
                    message,
                )?;
                let context =
                    constraint_context_store_from_v2_map(self.contexts, message, "contexts")?;
                let owned_ids = active
                    .keys()
                    .chain(inactive.keys())
                    .copied()
                    .collect::<BTreeSet<_>>();
                validate_context_reference_ids(&context, &owned_ids)
                    .map_err(|e| ParseError::new(e).context(message, "contexts"))?;
                ConstraintCollection::with_context(active, inactive, context)
                    .map_err(|e| ParseError::new(e).context(message, "active"))
            }
        }
    };
}

impl_parse_v2_created_collection!(Constraint => crate::v2::RegularConstraintCollection);
impl_parse_v2_created_collection!(crate::IndicatorConstraint => crate::v2::IndicatorConstraintCollection);
impl_parse_v2_created_collection!(crate::OneHotConstraint => crate::v2::OneHotConstraintCollection);
impl_parse_v2_created_collection!(crate::Sos1Constraint => crate::v2::Sos1ConstraintCollection);

macro_rules! impl_parse_v2_evaluated_collection {
    ($source:ty => $target:ty) => {
        impl Parse for $target {
            type Output = EvaluatedCollection<$source>;
            type Context = crate::ATol;

            fn parse(self, atol: &Self::Context) -> std::result::Result<Self::Output, ParseError> {
                let message = stringify!($target);
                let entries = parse_v2_constraint_entries_with_context(
                    self.entries,
                    message,
                    "entries",
                    atol,
                )?;
                let inactive_metadata = parse_v2_inactive_metadata(
                    self.inactive_metadata,
                    message,
                    "inactive_metadata",
                )?;
                let context =
                    constraint_context_store_from_v2_map(self.contexts, message, "contexts")?;
                let owned_ids = entries.keys().copied().collect::<BTreeSet<_>>();
                validate_context_reference_ids(&context, &owned_ids)
                    .map_err(|e| ParseError::new(e).context(message, "contexts"))?;
                EvaluatedCollection::with_context(entries, inactive_metadata, context)
                    .map_err(|e| ParseError::new(e).context(message, "inactive_metadata"))
            }
        }
    };
}

impl_parse_v2_evaluated_collection!(Constraint => crate::v2::EvaluatedRegularConstraintCollection);
impl_parse_v2_evaluated_collection!(crate::IndicatorConstraint => crate::v2::EvaluatedIndicatorConstraintCollection);

macro_rules! impl_parse_v2_sampled_collection {
    ($source:ty => $target:ty) => {
        impl Parse for $target {
            type Output = SampledCollection<$source>;
            type Context = crate::ATol;

            fn parse(self, atol: &Self::Context) -> std::result::Result<Self::Output, ParseError> {
                let message = stringify!($target);
                let entries = parse_v2_constraint_entries_with_context(
                    self.entries,
                    message,
                    "entries",
                    atol,
                )?;
                let inactive_metadata = parse_v2_inactive_metadata(
                    self.inactive_metadata,
                    message,
                    "inactive_metadata",
                )?;
                let context =
                    constraint_context_store_from_v2_map(self.contexts, message, "contexts")?;
                let owned_ids = entries.keys().copied().collect::<BTreeSet<_>>();
                validate_context_reference_ids(&context, &owned_ids)
                    .map_err(|e| ParseError::new(e).context(message, "contexts"))?;
                SampledCollection::with_context(entries, inactive_metadata, context)
                    .map_err(|e| ParseError::new(e).context(message, "inactive_metadata"))
            }
        }
    };
}

impl_parse_v2_sampled_collection!(Constraint => crate::v2::SampledRegularConstraintCollection);
impl_parse_v2_sampled_collection!(crate::IndicatorConstraint => crate::v2::SampledIndicatorConstraintCollection);

// Solution and SampleSet supply the cross-table values required to restore a
// structural constraint's metric. The collection owns its IDs and sidecars.
macro_rules! impl_restore_v2_structural_collection {
    ($source:ty => $evaluated:ty, $sampled:ty) => {
        impl $evaluated {
            pub(crate) fn parse_with_values(
                self,
                variables: &crate::EvaluatedDecisionVariableTable,
                atol: ATol,
            ) -> std::result::Result<EvaluatedCollection<$source>, ParseError> {
                let message = stringify!($evaluated);
                let entries = self
                    .entries
                    .into_iter()
                    .map(|(id, row)| {
                        let constraint = row
                            .parse_with_values(
                                |variable_id| {
                                    variables
                                        .get(&variable_id)
                                        .map(|variable| *variable.value())
                                },
                                atol,
                            )
                            .map_err(|error| error.context(message, "entries"))?;
                        Ok((<<$source as ConstraintType>::ID>::from(id), constraint))
                    })
                    .collect::<std::result::Result<BTreeMap<_, _>, ParseError>>()?;
                let inactive_metadata = parse_v2_inactive_metadata(
                    self.inactive_metadata,
                    message,
                    "inactive_metadata",
                )?;
                let context =
                    constraint_context_store_from_v2_map(self.contexts, message, "contexts")?;
                let owned_ids = entries.keys().copied().collect::<BTreeSet<_>>();
                validate_context_reference_ids(&context, &owned_ids)
                    .map_err(|error| ParseError::new(error).context(message, "contexts"))?;
                EvaluatedCollection::with_context(entries, inactive_metadata, context)
                    .map_err(|error| ParseError::new(error).context(message, "inactive_metadata"))
            }
        }

        impl $sampled {
            pub(crate) fn parse_with_values(
                self,
                variables: &crate::SampledDecisionVariableTable,
                atol: ATol,
            ) -> std::result::Result<SampledCollection<$source>, ParseError> {
                let message = stringify!($sampled);
                let entries = self
                    .entries
                    .into_iter()
                    .map(|(id, row)| {
                        let constraint = row
                            .parse_with_values(
                                |sample_id, variable_id| {
                                    variables
                                        .get(&variable_id)
                                        .and_then(|variable| variable.samples().get(sample_id))
                                        .copied()
                                },
                                atol,
                            )
                            .map_err(|error| error.context(message, "entries"))?;
                        Ok((<<$source as ConstraintType>::ID>::from(id), constraint))
                    })
                    .collect::<std::result::Result<BTreeMap<_, _>, ParseError>>()?;
                let inactive_metadata = parse_v2_inactive_metadata(
                    self.inactive_metadata,
                    message,
                    "inactive_metadata",
                )?;
                let context =
                    constraint_context_store_from_v2_map(self.contexts, message, "contexts")?;
                let owned_ids = entries.keys().copied().collect::<BTreeSet<_>>();
                validate_context_reference_ids(&context, &owned_ids)
                    .map_err(|error| ParseError::new(error).context(message, "contexts"))?;
                SampledCollection::with_context(entries, inactive_metadata, context)
                    .map_err(|error| ParseError::new(error).context(message, "inactive_metadata"))
            }
        }
    };
}

impl_restore_v2_structural_collection!(crate::OneHotConstraint => crate::v2::EvaluatedOneHotConstraintCollection, crate::v2::SampledOneHotConstraintCollection);
impl_restore_v2_structural_collection!(crate::Sos1Constraint => crate::v2::EvaluatedSos1ConstraintCollection, crate::v2::SampledSos1ConstraintCollection);

fn entries_to_v2_map<ID, Row, V2Row>(entries: BTreeMap<ID, Row>) -> BTreeMap<u64, V2Row>
where
    ID: IDType,
    Row: Into<V2Row>,
{
    entries
        .into_iter()
        .map(|(id, row)| (id.into(), row.into()))
        .collect()
}

fn parse_v2_constraint_entries<ID, V2Row, Row>(
    entries: BTreeMap<u64, V2Row>,
    message: &'static str,
    field: &'static str,
) -> std::result::Result<BTreeMap<ID, Row>, ParseError>
where
    ID: IDType,
    V2Row: Parse<Output = Row, Context = ()>,
{
    entries
        .into_iter()
        .map(|(id, row)| Ok((ID::from(id), row.parse_as(&(), message, field)?)))
        .collect()
}

fn parse_v2_constraint_entries_with_context<ID, V2Row, Row, C>(
    entries: BTreeMap<u64, V2Row>,
    message: &'static str,
    field: &'static str,
    context: &C,
) -> std::result::Result<BTreeMap<ID, Row>, ParseError>
where
    ID: IDType,
    V2Row: Parse<Output = Row, Context = C>,
{
    entries
        .into_iter()
        .map(|(id, row)| Ok((ID::from(id), row.parse_as(context, message, field)?)))
        .collect()
}

fn inactive_entries_to_v2_maps<ID, Row, V2Row>(
    inactive: BTreeMap<ID, (Row, InactiveConstraintMetadata)>,
) -> (
    BTreeMap<u64, V2Row>,
    BTreeMap<u64, crate::v2::InactiveConstraintMetadata>,
)
where
    ID: IDType,
    Row: Into<V2Row>,
{
    let mut rows = BTreeMap::new();
    let mut reasons = BTreeMap::new();
    for (id, (row, reason)) in inactive {
        let id = id.into();
        rows.insert(id, row.into());
        reasons.insert(id, reason.into());
    }
    (rows, reasons)
}

fn parse_v2_inactive_constraint_entries<ID, V2Row, Row>(
    inactive: BTreeMap<u64, V2Row>,
    mut inactive_metadata: BTreeMap<u64, crate::v2::InactiveConstraintMetadata>,
    message: &'static str,
) -> std::result::Result<BTreeMap<ID, (Row, InactiveConstraintMetadata)>, ParseError>
where
    ID: IDType,
    V2Row: Parse<Output = Row, Context = ()>,
{
    let mut out = BTreeMap::new();
    for (id, row) in inactive {
        let reason = inactive_metadata.remove(&id).ok_or_else(|| {
            ParseError::new(crate::error!(
                "Inactive constraint ID {:?} has no inactive metadata",
                ID::from(id)
            ))
            .context(message, "inactive_metadata")
        })?;
        out.insert(
            ID::from(id),
            (row.parse_as(&(), message, "inactive")?, reason.into()),
        );
    }
    if let Some(id) = inactive_metadata.keys().next().copied() {
        return Err(ParseError::new(crate::error!(
            "Inactive metadata references unknown constraint ID {:?}",
            ID::from(id)
        ))
        .context(message, "inactive_metadata"));
    }
    Ok(out)
}

fn inactive_metadata_to_v2_map<ID: IDType>(
    inactive_metadata: BTreeMap<ID, InactiveConstraintMetadata>,
) -> BTreeMap<u64, crate::v2::InactiveConstraintMetadata> {
    inactive_metadata
        .into_iter()
        .map(|(id, reason)| (id.into(), reason.into()))
        .collect()
}

fn parse_v2_inactive_metadata<ID: IDType>(
    inactive_metadata: BTreeMap<u64, crate::v2::InactiveConstraintMetadata>,
    _message: &'static str,
    _field: &'static str,
) -> std::result::Result<BTreeMap<ID, InactiveConstraintMetadata>, ParseError> {
    Ok(inactive_metadata
        .into_iter()
        .map(|(id, reason)| (ID::from(id), reason.into()))
        .collect())
}

impl<T: ConstraintType> Evaluate for ConstraintCollection<T> {
    type Output = EvaluatedCollection<T>;
    type SampledOutput = SampledCollection<T>;

    fn evaluate(&self, state: &v1::State, atol: ATol) -> Result<Self::Output> {
        let mut results = BTreeMap::new();
        let mut inactive_metadata = BTreeMap::new();
        for (id, constraint) in &self.active {
            let evaluated = constraint.evaluate(state, atol).inspect_err(|e| {
                tracing::error!(?id, error = %e, "failed to evaluate active constraint");
            })?;
            results.insert(*id, evaluated);
        }
        for (id, (constraint, reason)) in &self.inactive {
            let evaluated = constraint.evaluate(state, atol).inspect_err(|e| {
                tracing::error!(?id, error = %e, "failed to evaluate inactive constraint");
            })?;
            results.insert(*id, evaluated);
            inactive_metadata.insert(*id, reason.clone());
        }
        EvaluatedCollection::with_context(results, inactive_metadata, self.context.clone())
    }

    fn evaluate_samples(
        &self,
        samples: &crate::Sampled<v1::State>,
        atol: ATol,
    ) -> Result<Self::SampledOutput> {
        let mut results = BTreeMap::new();
        let mut inactive_metadata = BTreeMap::new();
        for (id, constraint) in &self.active {
            let evaluated = constraint.evaluate_samples(samples, atol).inspect_err(|e| {
                tracing::error!(?id, error = %e, "failed to evaluate_samples active constraint");
            })?;
            results.insert(*id, evaluated);
        }
        for (id, (constraint, reason)) in &self.inactive {
            let evaluated = constraint.evaluate_samples(samples, atol).inspect_err(|e| {
                tracing::error!(?id, error = %e, "failed to evaluate_samples inactive constraint");
            })?;
            results.insert(*id, evaluated);
            inactive_metadata.insert(*id, reason.clone());
        }
        SampledCollection::with_context(results, inactive_metadata, self.context.clone())
    }

    fn partial_evaluate(&mut self, state: &v1::State, atol: ATol) -> Result<()> {
        for (id, constraint) in self.active.iter_mut() {
            constraint.partial_evaluate(state, atol).inspect_err(|e| {
                tracing::error!(?id, error = %e, "failed to partial_evaluate constraint");
            })?;
        }
        Ok(())
    }

    /// IDs needed to evaluate every retained row, including skipped inactive rows.
    ///
    /// A collection does not populate fixed, dependent, or irrelevant values;
    /// its caller supplies the values referenced by all rows.
    fn required_ids(&self) -> VariableIDSet {
        let mut ids = VariableIDSet::default();
        for constraint in self.values() {
            ids.extend(constraint.required_ids());
        }
        ids
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
/// This result table owns only evaluated rows, inactive metadata, and context
/// sidecars for one constraint family. It validates that inactive-metadata and
/// context IDs refer to existing evaluated rows, then remains effectively
/// read-oriented: construction, row/sidecar reads, feasibility queries,
/// inactive-state queries, host-owned by-value replacement when required, and
/// consumption at conversion boundaries. Global consistency with evaluated
/// decision-variable rows and named functions belongs to [`crate::Solution`].
#[derive(Debug, Clone, PartialEq)]
pub struct EvaluatedCollection<T: ConstraintType> {
    constraints: BTreeMap<T::ID, T::Evaluated>,
    inactive_metadata: BTreeMap<T::ID, InactiveConstraintMetadata>,
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
            inactive_metadata: BTreeMap::new(),
            context: ConstraintContextStore::default(),
        }
    }
}

impl<T: ConstraintType> EvaluatedCollection<T> {
    /// Construct an evaluated collection without label/provenance sidecars.
    ///
    /// # Errors
    ///
    /// Returns an error if `inactive_metadata` contains an ID that is not present in
    /// `constraints`.
    pub fn new(
        constraints: BTreeMap<T::ID, T::Evaluated>,
        inactive_metadata: BTreeMap<T::ID, InactiveConstraintMetadata>,
    ) -> crate::Result<Self> {
        validate_inactive_metadata_reference_entries(&constraints, &inactive_metadata)?;
        Ok(Self {
            constraints,
            inactive_metadata,
            context: ConstraintContextStore::default(),
        })
    }

    /// Construct an evaluated collection together with its label/provenance store.
    /// Used by [`ConstraintCollection::evaluate`] to thread the source
    /// collection's sidecars through unchanged.
    ///
    /// # Errors
    ///
    /// Returns an error if `inactive_metadata` contains an ID that is not present in
    /// `constraints`.
    pub fn with_context(
        constraints: BTreeMap<T::ID, T::Evaluated>,
        inactive_metadata: BTreeMap<T::ID, InactiveConstraintMetadata>,
        context: ConstraintContextStore<T::ID>,
    ) -> crate::Result<Self> {
        validate_inactive_metadata_reference_entries(&constraints, &inactive_metadata)?;
        let owned_ids = constraints.keys().copied().collect::<BTreeSet<_>>();
        validate_context_reference_ids(&context, &owned_ids)?;
        Ok(Self {
            constraints,
            inactive_metadata,
            context,
        })
    }

    pub fn inner(&self) -> &BTreeMap<T::ID, T::Evaluated> {
        &self.constraints
    }

    /// Replace an evaluated row while preserving inactive-state and context sidecars.
    ///
    /// Returns [`None`] when `id` is unknown to this collection.
    pub(crate) fn replace_evaluated_row(
        &mut self,
        id: T::ID,
        constraint: T::Evaluated,
    ) -> Option<T::Evaluated> {
        match self.constraints.entry(id) {
            Entry::Occupied(mut entry) => Some(entry.insert(constraint)),
            Entry::Vacant(_) => None,
        }
    }

    /// Access the inactive metadata map.
    pub fn inactive_metadata(&self) -> &BTreeMap<T::ID, InactiveConstraintMetadata> {
        &self.inactive_metadata
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

    /// Check if a constraint is inactive.
    pub fn is_inactive(&self, id: &T::ID) -> bool {
        self.inactive_metadata.contains_key(id)
    }

    pub fn is_empty(&self) -> bool {
        self.constraints.is_empty()
    }

    /// Participation of an evaluated row, or `None` for an unknown ID.
    pub fn activeness(&self, id: &T::ID) -> Option<crate::ConstraintActiveness> {
        self.constraints.contains_key(id).then(|| {
            self.inactive_metadata.get(id).map_or(
                crate::ConstraintActiveness::Active,
                InactiveConstraintMetadata::activeness,
            )
        })
    }

    /// Rows at or above the requested minimum activeness.
    pub fn selected(
        &self,
        min_activeness: crate::ConstraintActiveness,
    ) -> impl Iterator<Item = (&T::ID, &T::Evaluated)> {
        self.constraints
            .iter()
            .filter(move |(id, _)| self.activeness(id).is_some_and(|a| a >= min_activeness))
    }

    /// Check the selected constraints using their individual violation metrics.
    pub fn is_feasible(&self, atol: ATol, min_activeness: crate::ConstraintActiveness) -> bool {
        self.selected(min_activeness)
            .all(|(_, c)| c.is_feasible(atol))
    }

    /// Check if all non-inactive constraints are feasible.
    pub fn is_feasible_relaxed(&self, atol: ATol) -> bool {
        self.is_feasible(atol, crate::ConstraintActiveness::Active)
    }
}

impl TryFrom<EvaluatedCollection<Constraint>> for Vec<v1::EvaluatedConstraint> {
    type Error = crate::Error;

    fn try_from(value: EvaluatedCollection<Constraint>) -> crate::Result<Self> {
        crate::v1_io::validate_inactive_metadata(value.inactive_metadata().values())?;
        let EvaluatedCollection {
            constraints,
            mut inactive_metadata,
            mut context,
        } = value;
        Ok(constraints
            .into_iter()
            .map(|(id, constraint)| {
                let context = context.remove(id);
                match inactive_metadata.remove(&id) {
                    Some(reason) => evaluated_constraint_to_v1(id, constraint, context, reason),
                    None => evaluated_constraint_to_v1_active(id, constraint, context),
                }
            })
            .collect())
    }
}

fn evaluated_constraint_to_v1_active(
    id: ConstraintID,
    constraint: EvaluatedConstraint,
    context: ConstraintContext,
) -> v1::EvaluatedConstraint {
    let label = context.label;
    v1::EvaluatedConstraint {
        id: id.into_inner(),
        equality: constraint.equality.into(),
        evaluated_value: constraint.stage.evaluated_value,
        used_decision_variable_ids: constraint
            .stage
            .used_decision_variable_ids
            .into_iter()
            .map(|id| id.into_inner())
            .collect(),
        subscripts: label.subscripts,
        parameters: label.parameters.into_iter().collect(),
        name: label.name,
        description: label.description,
        dual_variable: constraint.stage.dual_variable,
        removed_reason: None,
        removed_reason_parameters: Default::default(),
    }
}

fn evaluated_constraint_to_v1(
    id: ConstraintID,
    constraint: EvaluatedConstraint,
    context: ConstraintContext,
    inactive_metadata: InactiveConstraintMetadata,
) -> v1::EvaluatedConstraint {
    let mut value = evaluated_constraint_to_v1_active(id, constraint, context);
    value.removed_reason = Some(inactive_metadata.reason);
    value.removed_reason_parameters = inactive_metadata.parameters.into_iter().collect();
    value
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
/// This result table owns only sampled rows, inactive metadata, and context
/// sidecars for one constraint family. It validates that inactive-metadata and
/// context IDs refer to existing sampled rows, exposes read and feasibility
/// queries, validates sampled-row sample IDs against a host-supplied sample set,
/// validates used decision-variable IDs against a host-supplied variable set,
/// and can be consumed at conversion boundaries. Global sample consistency
/// across tables belongs to [`crate::SampleSet`].
#[derive(Debug, Clone)]
pub struct SampledCollection<T: ConstraintType> {
    constraints: BTreeMap<T::ID, T::Sampled>,
    inactive_metadata: BTreeMap<T::ID, InactiveConstraintMetadata>,
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
            inactive_metadata: BTreeMap::new(),
            context: ConstraintContextStore::default(),
        }
    }
}

impl<T: ConstraintType> SampledCollection<T> {
    /// Construct a sampled collection without label/provenance sidecars.
    ///
    /// # Errors
    ///
    /// Returns an error if `inactive_metadata` contains an ID that is not present in
    /// `constraints`.
    pub fn new(
        constraints: BTreeMap<T::ID, T::Sampled>,
        inactive_metadata: BTreeMap<T::ID, InactiveConstraintMetadata>,
    ) -> crate::Result<Self> {
        validate_inactive_metadata_reference_entries(&constraints, &inactive_metadata)?;
        Ok(Self {
            constraints,
            inactive_metadata,
            context: ConstraintContextStore::default(),
        })
    }

    /// Construct a sampled collection together with its label/provenance store.
    /// Used by [`ConstraintCollection::evaluate_samples`] to thread the
    /// source collection's sidecars through unchanged.
    ///
    /// # Errors
    ///
    /// Returns an error if `inactive_metadata` contains an ID that is not present in
    /// `constraints`.
    pub fn with_context(
        constraints: BTreeMap<T::ID, T::Sampled>,
        inactive_metadata: BTreeMap<T::ID, InactiveConstraintMetadata>,
        context: ConstraintContextStore<T::ID>,
    ) -> crate::Result<Self> {
        validate_inactive_metadata_reference_entries(&constraints, &inactive_metadata)?;
        let owned_ids = constraints.keys().copied().collect::<BTreeSet<_>>();
        validate_context_reference_ids(&context, &owned_ids)?;
        Ok(Self {
            constraints,
            inactive_metadata,
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

    /// Access the inactive metadata map.
    pub fn inactive_metadata(&self) -> &BTreeMap<T::ID, InactiveConstraintMetadata> {
        &self.inactive_metadata
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

    /// Check if a constraint is inactive.
    pub fn is_inactive(&self, id: &T::ID) -> bool {
        self.inactive_metadata.contains_key(id)
    }

    pub fn is_empty(&self) -> bool {
        self.constraints.is_empty()
    }

    /// Participation of a sampled row, or `None` for an unknown ID.
    pub fn activeness(&self, id: &T::ID) -> Option<crate::ConstraintActiveness> {
        self.constraints.contains_key(id).then(|| {
            self.inactive_metadata.get(id).map_or(
                crate::ConstraintActiveness::Active,
                InactiveConstraintMetadata::activeness,
            )
        })
    }

    /// Rows at or above the requested minimum activeness.
    pub fn selected(
        &self,
        min_activeness: crate::ConstraintActiveness,
    ) -> impl Iterator<Item = (&T::ID, &T::Sampled)> {
        self.constraints
            .iter()
            .filter(move |(id, _)| self.activeness(id).is_some_and(|a| a >= min_activeness))
    }

    /// Check selected constraints for a sample.
    pub fn is_feasible_for(
        &self,
        sample_id: SampleID,
        atol: ATol,
        min_activeness: crate::ConstraintActiveness,
    ) -> bool {
        self.selected(min_activeness)
            .all(|(_, c)| c.is_feasible_for(sample_id, atol).unwrap_or(false))
    }

    /// Check if all non-inactive constraints are feasible for a given sample.
    pub fn is_feasible_relaxed_for(&self, sample_id: SampleID, atol: ATol) -> bool {
        self.is_feasible_for(sample_id, atol, crate::ConstraintActiveness::Active)
    }
}

impl SampledCollection<Constraint> {
    /// SampleSet supplies the v1 parser's default tolerance so row feasibility
    /// maps use the same conditions as the serialized root maps.
    pub(crate) fn into_v1(self, atol: ATol) -> crate::Result<Vec<v1::SampledConstraint>> {
        crate::v1_io::validate_inactive_metadata(self.inactive_metadata().values())?;
        let value = self;
        let SampledCollection {
            constraints,
            mut inactive_metadata,
            mut context,
        } = value;
        Ok(constraints
            .into_iter()
            .map(|(id, constraint)| {
                let context = context.remove(id);
                match inactive_metadata.remove(&id) {
                    Some(reason) => sampled_constraint_to_v1(id, constraint, context, reason, atol),
                    None => sampled_constraint_to_v1_active(id, constraint, context, atol),
                }
            })
            .collect())
    }
}

fn sampled_constraint_to_v1_active(
    id: ConstraintID,
    constraint: SampledConstraint,
    context: ConstraintContext,
    atol: ATol,
) -> v1::SampledConstraint {
    let label = context.label;
    let feasible = constraint
        .stage
        .evaluated_values
        .iter()
        .map(|(id, _)| {
            (
                id.into_inner(),
                constraint
                    .is_feasible_for(*id, atol)
                    .expect("sample exists"),
            )
        })
        .collect();
    let evaluated_values: v1::SampledValues = constraint.stage.evaluated_values.into();

    v1::SampledConstraint {
        id: id.into_inner(),
        equality: constraint.equality.into(),
        name: label.name,
        subscripts: label.subscripts,
        parameters: label.parameters.into_iter().collect(),
        description: label.description,
        removed_reason: None,
        removed_reason_parameters: Default::default(),
        evaluated_values: Some(evaluated_values),
        used_decision_variable_ids: constraint
            .stage
            .used_decision_variable_ids
            .into_iter()
            .map(|id| id.into_inner())
            .collect(),
        feasible,
    }
}

fn sampled_constraint_to_v1(
    id: ConstraintID,
    constraint: SampledConstraint,
    context: ConstraintContext,
    inactive_metadata: InactiveConstraintMetadata,
    atol: ATol,
) -> v1::SampledConstraint {
    let mut value = sampled_constraint_to_v1_active(id, constraint, context, atol);
    value.removed_reason = Some(inactive_metadata.reason);
    value.removed_reason_parameters = inactive_metadata.parameters.into_iter().collect();
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, constraint::ConstraintID, linear, Equality, Function, ModelingLabel};

    fn parse_error_source(error: &ParseError) -> &(dyn std::error::Error + 'static) {
        std::error::Error::source(error).expect("ParseError should expose its cause")
    }

    fn assert_ordinary_collection_sidecar_error(
        error: &ParseError,
        expected_message: &'static str,
        expected_field: &'static str,
    ) {
        let source = parse_error_source(error);
        assert!(source.downcast_ref::<crate::RawParseError>().is_none());
        assert!(source.downcast_ref::<crate::SolutionError>().is_none());
        assert!(source.downcast_ref::<crate::SampleSetError>().is_none());
        assert!(source.to_string().contains("unknown constraint ID"));

        assert_eq!(error.context.len(), 1);
        assert_eq!(error.context[0].message, expected_message);
        assert_eq!(error.context[0].field, expected_field);
        assert!(error
            .to_string()
            .contains(&format!("{expected_message}[{expected_field}]")));
    }

    fn inactive_metadata() -> InactiveConstraintMetadata {
        InactiveConstraintMetadata {
            reason: "test".to_string(),
            parameters: Default::default(),
            skip_feasibility_check: false,
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
        assert!(collection.inactive().is_empty());
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
        assert!(!results[&ConstraintID::from(1)].is_feasible(crate::ATol::default()));
        assert!(!results[&ConstraintID::from(2)].is_feasible(crate::ATol::default()));
        assert!(results.inactive_metadata().is_empty());
    }

    fn assert_collection_required_ids<T: ConstraintType>(make_row: impl Fn(u64) -> T::Created) {
        // Every row shares variable 0; variables 2 and 3 occur only in inactive
        // rows. Skipping feasibility never skips evaluation of those rows.
        let collection = ConstraintCollection::<T>::new(
            BTreeMap::from([(T::ID::from(1), make_row(1))]),
            BTreeMap::from([
                (T::ID::from(2), (make_row(2), inactive_metadata())),
                (
                    T::ID::from(3),
                    (
                        make_row(3),
                        InactiveConstraintMetadata {
                            skip_feasibility_check: true,
                            ..inactive_metadata()
                        },
                    ),
                ),
            ]),
        )
        .unwrap();
        assert_eq!(
            collection.active_decision_variable_ids(),
            VariableIDSet::from([0.into(), 1.into()]),
        );
        let required = Evaluate::required_ids(&collection);
        assert_eq!(required, (0..=3).map(crate::VariableID::from).collect());

        let state = v1::State::from_iter(required.iter().map(|id| {
            let value = if id.into_inner() == 0 { 0.0 } else { 1.0 };
            (id.into_inner(), value)
        }));
        let atol = ATol::default();
        assert_eq!(collection.evaluate(&state, atol).unwrap().len(), 3);
        let samples = crate::Sampled::from((SampleID::from(7), state.clone()));
        assert_eq!(
            collection.evaluate_samples(&samples, atol).unwrap().len(),
            3
        );

        for missing_id in [2, 3] {
            let mut incomplete = state.clone();
            incomplete.entries.remove(&missing_id);
            assert!(collection.evaluate(&incomplete, atol).is_err());
            let samples = crate::Sampled::new_no_dedup(
                [
                    (SampleID::from(7), state.clone()),
                    (SampleID::from(9), incomplete),
                ]
                .into_iter(),
            );
            assert!(collection.evaluate_samples(&samples, atol).is_err());
        }
    }

    #[test]
    fn regular_collection_required_ids_include_checked_and_skipped_rows() {
        assert_collection_required_ids::<Constraint>(|id| {
            Constraint::equal_to_zero(Function::from(linear!(0) + linear!(id)))
        });
    }

    #[test]
    fn indicator_collection_required_ids_include_checked_and_skipped_rows() {
        assert_collection_required_ids::<crate::IndicatorConstraint>(|id| {
            crate::IndicatorConstraint::new(
                0.into(),
                Equality::EqualToZero,
                Function::from(linear!(id)),
            )
        });
    }

    #[test]
    fn one_hot_collection_required_ids_include_checked_and_skipped_rows() {
        assert_collection_required_ids::<crate::OneHotConstraint>(|id| {
            crate::OneHotConstraint::new(BTreeSet::from([0.into(), id.into()])).unwrap()
        });
    }

    #[test]
    fn sos1_collection_required_ids_include_checked_and_skipped_rows() {
        assert_collection_required_ids::<crate::Sos1Constraint>(|id| {
            crate::Sos1Constraint::new(BTreeSet::from([0.into(), id.into()])).unwrap()
        });
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
        assert_eq!(collection.inactive().len(), 0);
    }

    #[test]
    fn collection_rejects_active_inactive_overlap() {
        let id = ConstraintID::from(1);
        let active = BTreeMap::from([(id, Constraint::equal_to_zero(Function::Zero))]);
        let inactive = BTreeMap::from([(
            id,
            (
                Constraint::equal_to_zero(Function::Zero),
                inactive_metadata(),
            ),
        )]);

        let err = ConstraintCollection::<Constraint>::new(active, inactive).unwrap_err();
        assert!(err
            .to_string()
            .contains("appears in both active constraints and inactive constraints"));
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
    fn replace_and_remove_active_rows_preserves_context() {
        let inactive_id = ConstraintID::from(1);
        let active_id = ConstraintID::from(2);
        let active = BTreeMap::from([
            (inactive_id, Constraint::equal_to_zero(Function::Zero)),
            (
                active_id,
                Constraint::equal_to_zero(Function::from(linear!(1))),
            ),
        ]);
        let mut context = ConstraintContextStore::default();
        context.set_name(inactive_id, "original");
        let mut collection =
            ConstraintCollection::<Constraint>::with_context(active, BTreeMap::new(), context)
                .unwrap();

        let inactive = collection.active().get(&inactive_id).unwrap().clone();
        collection
            .replace_and_remove_active_rows(
                BTreeMap::new(),
                BTreeMap::from([(inactive_id, (inactive, inactive_metadata()))]),
            )
            .unwrap();

        assert!(!collection.active().contains_key(&inactive_id));
        assert!(collection.active().contains_key(&active_id));
        assert!(collection.inactive().contains_key(&inactive_id));
        assert_eq!(collection.context().name(inactive_id), Some("original"));
        collection.validate_context_ids().unwrap();
    }

    #[test]
    fn rewrite_active_rows_by_value_preserves_context_and_lifecycle() {
        let inactive_id = ConstraintID::from(1);
        let active_id = ConstraintID::from(2);
        let previously_inactive_id = ConstraintID::from(3);
        let active = BTreeMap::from([
            (
                inactive_id,
                Constraint::equal_to_zero(Function::from(linear!(1))),
            ),
            (
                active_id,
                Constraint::equal_to_zero(Function::from(linear!(2))),
            ),
        ]);
        let inactive = BTreeMap::from([(
            previously_inactive_id,
            (
                Constraint::equal_to_zero(Function::from(linear!(3))),
                inactive_metadata(),
            ),
        )]);
        let mut context = ConstraintContextStore::default();
        context.set_name(inactive_id, "moved");
        context.set_name(active_id, "active");
        context.set_name(previously_inactive_id, "previously inactive");
        let collection =
            ConstraintCollection::<Constraint>::with_context(active, inactive, context).unwrap();

        let collection = collection
            .rewrite_active_rows_by_value(|id, row| {
                Ok(if id == inactive_id {
                    ActiveRowRewrite::Inactive(row, inactive_metadata())
                } else {
                    ActiveRowRewrite::Active(row)
                })
            })
            .unwrap();

        assert!(!collection.active().contains_key(&inactive_id));
        assert!(collection.active().contains_key(&active_id));
        assert!(collection.inactive().contains_key(&inactive_id));
        assert!(collection.inactive().contains_key(&previously_inactive_id));
        assert_eq!(collection.context().name(inactive_id), Some("moved"));
        assert_eq!(collection.context().name(active_id), Some("active"));
        assert_eq!(
            collection.context().name(previously_inactive_id),
            Some("previously inactive")
        );
        collection.validate_context_ids().unwrap();
    }

    #[test]
    fn rewrite_active_rows_by_value_propagates_error_without_partial_value() {
        let id = ConstraintID::from(1);
        let collection = ConstraintCollection::<Constraint>::new(
            BTreeMap::from([(id, Constraint::equal_to_zero(Function::from(linear!(1))))]),
            BTreeMap::new(),
        )
        .unwrap();

        let error = collection
            .rewrite_active_rows_by_value(|_id, _row| {
                Err(crate::error!("owned row rewrite failed"))
            })
            .unwrap_err();

        assert_eq!(error.to_string(), "owned row rewrite failed");
    }

    #[test]
    fn move_active_rows_with_reasons_rejects_unknown_id_before_mutation() {
        let active_id = ConstraintID::from(1);
        let unknown_id = ConstraintID::from(99);
        let mut collection = ConstraintCollection::<Constraint>::new(
            BTreeMap::from([(
                active_id,
                Constraint::equal_to_zero(Function::from(linear!(1)).abs()),
            )]),
            BTreeMap::new(),
        )
        .unwrap();
        let original = collection.clone();

        let error = collection
            .move_active_rows_to_inactive_with_reasons(BTreeMap::from([
                (active_id, inactive_metadata()),
                (unknown_id, inactive_metadata()),
            ]))
            .unwrap_err();

        assert!(error.to_string().contains("not found"));
        assert_eq!(collection, original);
    }

    #[test]
    fn move_active_rows_with_reasons_moves_payload_and_preserves_context() {
        let inactive_id = ConstraintID::from(1);
        let active_id = ConstraintID::from(2);
        let active = BTreeMap::from([
            (
                inactive_id,
                Constraint::equal_to_zero(Function::from(linear!(1)).abs()),
            ),
            (
                active_id,
                Constraint::equal_to_zero(Function::from(linear!(1))),
            ),
        ]);
        let mut context = ConstraintContextStore::default();
        context.set_name(inactive_id, "original");
        let mut collection =
            ConstraintCollection::<Constraint>::with_context(active, BTreeMap::new(), context)
                .unwrap();
        let Function::Expression(expression) =
            collection.active().get(&inactive_id).unwrap().function()
        else {
            panic!("absolute value must use expression storage")
        };
        let original_instructions = crate::function::operation::instructions(expression).as_ptr();

        collection
            .move_active_rows_to_inactive_with_reasons(BTreeMap::from([(
                inactive_id,
                inactive_metadata(),
            )]))
            .unwrap();

        let Function::Expression(expression) = collection
            .inactive()
            .get(&inactive_id)
            .unwrap()
            .0
            .function()
        else {
            panic!("moved constraint must preserve expression storage")
        };
        assert_eq!(
            crate::function::operation::instructions(expression).as_ptr(),
            original_instructions
        );
        assert!(collection.active().contains_key(&active_id));
        assert_eq!(collection.context().name(inactive_id), Some("original"));
        collection.validate_context_ids().unwrap();
    }

    #[test]
    fn replace_active_rows_rejects_unknown_id_before_mutation() {
        let id = ConstraintID::from(1);
        let orphan_id = ConstraintID::from(99);
        let original = Constraint::equal_to_zero(Function::Zero);
        let mut collection = ConstraintCollection::<Constraint>::new(
            BTreeMap::from([(id, original.clone())]),
            BTreeMap::new(),
        )
        .unwrap();

        let err = collection
            .replace_active_rows(BTreeMap::from([
                (id, Constraint::less_than_or_equal_to_zero(Function::Zero)),
                (orphan_id, Constraint::equal_to_zero(Function::Zero)),
            ]))
            .unwrap_err();

        assert!(
            err.to_string().contains("Active constraint with ID")
                && err.to_string().contains("ConstraintID(99)"),
            "unexpected error: {err}"
        );
        assert_eq!(collection.active().get(&id), Some(&original));
        assert!(collection.inactive().is_empty());
        collection.validate_context_ids().unwrap();
    }

    #[test]
    fn replace_rows_preserving_lifecycle_keeps_reasons_and_context() {
        let active_id = ConstraintID::from(1);
        let inactive_id = ConstraintID::from(2);
        let reason = inactive_metadata();
        let mut context = ConstraintContextStore::default();
        context.set_name(active_id, "active");
        context.set_name(inactive_id, "inactive");
        let mut collection = ConstraintCollection::<Constraint>::with_context(
            BTreeMap::from([(active_id, Constraint::equal_to_zero(Function::Zero))]),
            BTreeMap::from([(
                inactive_id,
                (
                    Constraint::less_than_or_equal_to_zero(Function::Zero),
                    reason.clone(),
                ),
            )]),
            context,
        )
        .unwrap();

        collection
            .replace_rows_preserving_lifecycle(
                BTreeMap::from([(
                    active_id,
                    Constraint::less_than_or_equal_to_zero(Function::Zero),
                )]),
                BTreeMap::from([(inactive_id, Constraint::equal_to_zero(Function::Zero))]),
            )
            .unwrap();

        assert_eq!(
            collection.active().get(&active_id).map(|c| c.equality),
            Some(Equality::LessThanOrEqualToZero)
        );
        assert_eq!(
            collection
                .inactive()
                .get(&inactive_id)
                .map(|(c, _)| c.equality),
            Some(Equality::EqualToZero)
        );
        assert_eq!(
            collection.inactive().get(&inactive_id).map(|(_, r)| r),
            Some(&reason)
        );
        assert_eq!(collection.context().name(active_id), Some("active"));
        assert_eq!(collection.context().name(inactive_id), Some("inactive"));
        collection.validate_context_ids().unwrap();
    }

    #[test]
    fn replace_rows_preserving_lifecycle_rejects_unknown_inactive_id_before_mutation() {
        let active_id = ConstraintID::from(1);
        let inactive_id = ConstraintID::from(2);
        let orphan_id = ConstraintID::from(99);
        let original_active = Constraint::equal_to_zero(Function::Zero);
        let original_inactive = Constraint::less_than_or_equal_to_zero(Function::Zero);
        let reason = inactive_metadata();
        let mut collection = ConstraintCollection::<Constraint>::new(
            BTreeMap::from([(active_id, original_active.clone())]),
            BTreeMap::from([(inactive_id, (original_inactive.clone(), reason.clone()))]),
        )
        .unwrap();

        let err = collection
            .replace_rows_preserving_lifecycle(
                BTreeMap::from([(
                    active_id,
                    Constraint::less_than_or_equal_to_zero(Function::Zero),
                )]),
                BTreeMap::from([(orphan_id, Constraint::equal_to_zero(Function::Zero))]),
            )
            .unwrap_err();

        assert!(
            err.to_string().contains("Inactive constraint with ID")
                && err.to_string().contains("ConstraintID(99)"),
            "unexpected error: {err}"
        );
        assert_eq!(collection.active().get(&active_id), Some(&original_active));
        assert_eq!(
            collection.inactive().get(&inactive_id),
            Some(&(original_inactive, reason))
        );
        collection.validate_context_ids().unwrap();
    }

    #[test]
    fn activate_row_preserves_context() {
        let id = ConstraintID::from(1);
        let inactive = Constraint::less_than_or_equal_to_zero(Function::Zero);
        let mut context = ConstraintContextStore::default();
        context.set_name(id, "activated");
        let mut collection = ConstraintCollection::<Constraint>::with_context(
            BTreeMap::new(),
            BTreeMap::from([(id, (inactive.clone(), inactive_metadata()))]),
            context,
        )
        .unwrap();

        let mut activated = inactive.clone();
        activated.equality = Equality::EqualToZero;
        let reason = collection.activate_row(id, activated).unwrap();
        assert_eq!(reason.reason, "test");
        assert!(collection
            .active()
            .get(&id)
            .is_some_and(|c| c.equality == Equality::EqualToZero));
        assert!(!collection.inactive().contains_key(&id));
        assert_eq!(collection.context().name(id), Some("activated"));
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

        let inactive_id = ConstraintID::from(2);
        let mut collection = ConstraintCollection::<Constraint>::new(
            BTreeMap::new(),
            BTreeMap::from([(
                inactive_id,
                (
                    Constraint::equal_to_zero(Function::Zero),
                    inactive_metadata(),
                ),
            )]),
        )
        .unwrap();

        let err = collection
            .insert_active_with_context(
                inactive_id,
                Constraint::equal_to_zero(Function::Zero),
                ConstraintContext::default(),
            )
            .unwrap_err();
        assert!(err.to_string().contains("already exists in the inactive"));
    }

    #[test]
    fn evaluated_collection_rejects_inactive_metadata_without_constraint() {
        let constraints: BTreeMap<ConstraintID, EvaluatedConstraint> = BTreeMap::new();
        let inactive_metadata = BTreeMap::from([(ConstraintID::from(1), inactive_metadata())]);

        let err =
            EvaluatedCollection::<Constraint>::new(constraints, inactive_metadata).unwrap_err();
        assert!(err
            .to_string()
            .contains("Inactive metadata references unknown constraint ID"));
    }

    #[test]
    fn sampled_collection_rejects_inactive_metadata_without_constraint() {
        let constraints: BTreeMap<ConstraintID, SampledConstraint> = BTreeMap::new();
        let inactive_metadata = BTreeMap::from([(ConstraintID::from(1), inactive_metadata())]);

        let err = SampledCollection::<Constraint>::new(constraints, inactive_metadata).unwrap_err();
        assert!(err
            .to_string()
            .contains("Inactive metadata references unknown constraint ID"));
    }

    #[test]
    fn parse_v2_created_collection_orphan_context_reports_contexts_field() {
        let err = crate::v2::RegularConstraintCollection {
            contexts: [(
                1,
                crate::v2::ConstraintContext {
                    label: Some(crate::v2::ModelingLabel {
                        name: Some("orphan".to_string()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )]
            .into_iter()
            .collect(),
            ..Default::default()
        }
        .parse(&())
        .unwrap_err();

        assert_ordinary_collection_sidecar_error(
            &err,
            "crate::v2::RegularConstraintCollection",
            "contexts",
        );
    }

    #[test]
    fn parse_v2_evaluated_collection_orphan_context_reports_contexts_field() {
        let atol = ATol::default();
        let err = crate::v2::EvaluatedRegularConstraintCollection {
            contexts: [(
                1,
                crate::v2::ConstraintContext {
                    label: Some(crate::v2::ModelingLabel {
                        name: Some("orphan".to_string()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )]
            .into_iter()
            .collect(),
            ..Default::default()
        }
        .parse(&atol)
        .unwrap_err();

        assert_ordinary_collection_sidecar_error(
            &err,
            "crate::v2::EvaluatedRegularConstraintCollection",
            "contexts",
        );
    }

    #[test]
    fn parse_v2_sampled_collection_orphan_context_reports_contexts_field() {
        let atol = ATol::default();
        let err = crate::v2::SampledRegularConstraintCollection {
            contexts: [(
                1,
                crate::v2::ConstraintContext {
                    label: Some(crate::v2::ModelingLabel {
                        name: Some("orphan".to_string()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )]
            .into_iter()
            .collect(),
            ..Default::default()
        }
        .parse(&atol)
        .unwrap_err();

        assert_ordinary_collection_sidecar_error(
            &err,
            "crate::v2::SampledRegularConstraintCollection",
            "contexts",
        );
    }

    #[test]
    fn parse_v2_evaluated_collection_reports_invalid_inactive_metadata_as_ordinary_error() {
        let err = crate::v2::EvaluatedRegularConstraintCollection {
            inactive_metadata: BTreeMap::from([(
                1,
                crate::v2::InactiveConstraintMetadata {
                    reason: "test".to_string(),
                    parameters: Default::default(),
                    skip_feasibility_check: false,
                },
            )]),
            ..Default::default()
        }
        .parse(&ATol::default())
        .unwrap_err();

        assert_ordinary_collection_sidecar_error(
            &err,
            "crate::v2::EvaluatedRegularConstraintCollection",
            "inactive_metadata",
        );
    }

    #[test]
    fn parse_v2_sampled_collection_reports_invalid_inactive_metadata_as_ordinary_error() {
        let err = crate::v2::SampledRegularConstraintCollection {
            inactive_metadata: BTreeMap::from([(
                1,
                crate::v2::InactiveConstraintMetadata {
                    reason: "test".to_string(),
                    parameters: Default::default(),
                    skip_feasibility_check: false,
                },
            )]),
            ..Default::default()
        }
        .parse(&ATol::default())
        .unwrap_err();

        assert_ordinary_collection_sidecar_error(
            &err,
            "crate::v2::SampledRegularConstraintCollection",
            "inactive_metadata",
        );
    }
}
