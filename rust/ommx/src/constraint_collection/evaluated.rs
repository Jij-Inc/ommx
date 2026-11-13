use super::*;
use crate::{ATol, EvaluatedConstraintBehavior};
use std::collections::btree_map::Entry;

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
    /// Used by [`ConstraintCollection::evaluate`](crate::Evaluate::evaluate) to thread the source
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

    /// Role of an evaluated row, or `None` for an unknown ID.
    pub fn role(&self, id: &T::ID) -> Option<crate::ConstraintRole> {
        self.constraints.contains_key(id).then(|| {
            self.inactive_metadata.get(id).map_or(
                crate::ConstraintRole::Active,
                InactiveConstraintMetadata::role,
            )
        })
    }

    /// Rows at or above the requested minimum role.
    pub fn selected(
        &self,
        min_role: crate::ConstraintRole,
    ) -> impl Iterator<Item = (&T::ID, &T::Evaluated)> {
        self.constraints
            .iter()
            .filter(move |(id, _)| self.role(id).is_some_and(|role| role >= min_role))
    }

    /// Check rows with `FeasibilityCheck` or `Active` roles using their violation metrics.
    /// Rows with the `EvaluationOnly` role do not participate in feasibility.
    pub fn is_feasible(&self, atol: ATol) -> bool {
        self.selected(crate::ConstraintRole::FeasibilityCheck)
            .all(|(_, c)| c.is_feasible(atol))
    }

    /// Check if all non-inactive constraints are feasible.
    pub fn is_feasible_relaxed(&self, atol: ATol) -> bool {
        self.selected(crate::ConstraintRole::Active)
            .all(|(_, c)| c.is_feasible(atol))
    }
}
