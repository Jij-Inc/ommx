use super::*;
use crate::{ConstraintContext, Evaluate, VariableIDSet};
use std::collections::btree_map::Entry;

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

    /// Look up a constraint's role independently of its family.
    pub fn role(&self, id: &T::ID) -> Option<crate::ConstraintRole> {
        if self.active.contains_key(id) {
            Some(crate::ConstraintRole::Active)
        } else {
            self.inactive.get(id).map(|(_, metadata)| metadata.role())
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

    /// Look up a retained row regardless of its role.
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
