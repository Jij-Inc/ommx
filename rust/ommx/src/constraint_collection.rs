//! Constraint collections own row membership, inactive metadata, and context sidecars.
//!
//! Root objects retain responsibility for cross-table invariants and coordinated
//! model transformations. Collections provide family-local storage and evaluation
//! operations.

mod created;
mod evaluate;
mod evaluated;
mod sampled;
mod v1;
mod v2;

#[cfg(test)]
mod tests;

use crate::{ConstraintContextStore, ConstraintType, IDType, InactiveConstraintMetadata};
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

/// Storage effect produced by a by-value rewrite of an active constraint row.
///
/// The enclosing domain owner decides which effect applies. The collection only
/// commits the corresponding lifecycle change while retaining the row ID and
/// its context sidecars.
pub(crate) enum ActiveRowRewrite<T> {
    Active(T),
    Inactive(T, InactiveConstraintMetadata),
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
/// It supports family-local operations:
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
/// - evaluation of active and inactive rows, preserving metadata and context;
/// - partial evaluation of active rows, preserving membership and sidecars;
/// - consuming active rows, inactive rows, and context at conversion boundaries.
///
/// It does not expose mutable row references or arbitrary active/inactive map
/// mutation. Its [`crate::Evaluate`] implementation evaluates or partially
/// evaluates this collection's rows without updating an enclosing model's
/// variable tables or other components. To transform an [`crate::Instance`] or
/// [`crate::ParametricInstance`], use its corresponding operation to coordinate
/// these changes and preserve cross-table invariants.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstraintCollection<T: ConstraintType> {
    active: BTreeMap<T::ID, T::Created>,
    inactive: BTreeMap<T::ID, (T::Created, InactiveConstraintMetadata)>,
    context: ConstraintContextStore<T::ID>,
}

/// A collection of evaluated constraints of a single type.
///
/// This is the Solution-side counterpart of [`ConstraintCollection`],
/// providing generic feasibility checks via [`crate::EvaluatedConstraintBehavior`].
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

/// A collection of sampled constraints of a single type.
///
/// This is the SampleSet-side counterpart of [`ConstraintCollection`],
/// providing generic per-sample feasibility checks via [`crate::SampledConstraintBehavior`].
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
