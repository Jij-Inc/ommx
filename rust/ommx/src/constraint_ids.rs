//! Shared constraint-number validation for model and result root owners.

use crate::{
    ConstraintCollection, ConstraintKind, ConstraintType, EvaluatedCollection, SampledCollection,
};
use std::collections::BTreeMap;

#[derive(Debug, thiserror::Error)]
#[error("Constraint number {number} is shared by {first_state} {first_family} and {second_state} {second_family} constraints; constraint numbers must be unique across all families, including inactive constraints")]
pub(crate) struct ConstraintNumberCollision {
    pub number: u64,
    pub first_family: &'static str,
    pub first_state: &'static str,
    pub second_family: &'static str,
    pub second_state: &'static str,
}

/// Transient validator used by Instance, ParametricInstance, Solution and SampleSet.
/// Collections cannot enforce uniqueness across families on their own.
#[derive(Default)]
pub(crate) struct ConstraintIds {
    owners: BTreeMap<u64, (ConstraintKind, &'static str)>,
}

impl ConstraintIds {
    fn insert(
        &mut self,
        id: u64,
        kind: ConstraintKind,
        inactive: bool,
    ) -> Result<(), ConstraintNumberCollision> {
        let state = if inactive { "inactive" } else { "active" };
        if let Some((previous_kind, previous_state)) = self.owners.insert(id, (kind, state)) {
            return Err(ConstraintNumberCollision {
                number: id,
                first_family: previous_kind.diagnostic_name(),
                first_state: previous_state,
                second_family: kind.diagnostic_name(),
                second_state: state,
            });
        }
        Ok(())
    }

    pub(crate) fn created<T: ConstraintType>(
        &mut self,
        collection: &ConstraintCollection<T>,
    ) -> Result<(), ConstraintNumberCollision> {
        for id in collection.active().keys() {
            self.insert((*id).into(), T::KIND, false)?;
        }
        for id in collection.inactive().keys() {
            self.insert((*id).into(), T::KIND, true)?;
        }
        Ok(())
    }

    pub(crate) fn evaluated<T: ConstraintType>(
        &mut self,
        collection: &EvaluatedCollection<T>,
    ) -> Result<(), ConstraintNumberCollision> {
        for id in collection.inner().keys() {
            self.insert(
                (*id).into(),
                T::KIND,
                collection.inactive_metadata().contains_key(id),
            )?;
        }
        Ok(())
    }

    pub(crate) fn sampled<T: ConstraintType>(
        &mut self,
        collection: &SampledCollection<T>,
    ) -> Result<(), ConstraintNumberCollision> {
        for id in collection.inner().keys() {
            self.insert(
                (*id).into(),
                T::KIND,
                collection.inactive_metadata().contains_key(id),
            )?;
        }
        Ok(())
    }
}
