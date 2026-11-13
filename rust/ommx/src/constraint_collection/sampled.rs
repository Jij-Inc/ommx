use super::*;
use crate::{ATol, SampleID, SampleIDSet, SampledConstraintBehavior, SampledConstraintData};

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
    /// Used by [`ConstraintCollection::evaluate_samples`](crate::Evaluate::evaluate_samples) to thread the
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

    /// Role of a sampled row, or `None` for an unknown ID.
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
    ) -> impl Iterator<Item = (&T::ID, &T::Sampled)> {
        self.constraints
            .iter()
            .filter(move |(id, _)| self.role(id).is_some_and(|role| role >= min_role))
    }

    /// Check rows with `FeasibilityCheck` or `Active` roles for a sample.
    /// Rows with the `EvaluationOnly` role do not participate in feasibility.
    pub fn is_feasible_for(&self, sample_id: SampleID, atol: ATol) -> bool {
        self.selected(crate::ConstraintRole::FeasibilityCheck)
            .all(|(_, c)| c.is_feasible_for(sample_id, atol).unwrap_or(false))
    }

    /// Check if all non-inactive constraints are feasible for a given sample.
    pub fn is_feasible_relaxed_for(&self, sample_id: SampleID, atol: ATol) -> bool {
        self.selected(crate::ConstraintRole::Active)
            .all(|(_, c)| c.is_feasible_for(sample_id, atol).unwrap_or(false))
    }
}
