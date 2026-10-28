//! Shared constraint-number validation for model and result root owners.

use crate::constraint_type::{
    ConstraintCollection, ConstraintType, EvaluatedCollection, SampledCollection,
};
use std::collections::BTreeMap;

#[derive(Debug, thiserror::Error)]
#[error("Constraint number {number} is shared by {first_state} {first_family} and {second_state} {second_family} constraints; constraint numbers must be unique across all families, including removed constraints")]
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
    owners: BTreeMap<u64, (&'static str, &'static str)>,
}

impl ConstraintIds {
    fn insert(
        &mut self,
        id: u64,
        family: &'static str,
        removed: bool,
    ) -> Result<(), ConstraintNumberCollision> {
        let state = if removed { "removed" } else { "active" };
        if let Some((previous_family, previous_state)) = self.owners.insert(id, (family, state)) {
            return Err(ConstraintNumberCollision {
                number: id,
                first_family: previous_family,
                first_state: previous_state,
                second_family: family,
                second_state: state,
            });
        }
        Ok(())
    }

    pub(crate) fn created<T: ConstraintType>(
        &mut self,
        family: &'static str,
        collection: &ConstraintCollection<T>,
    ) -> Result<(), ConstraintNumberCollision> {
        for id in collection.active().keys() {
            self.insert((*id).into(), family, false)?;
        }
        for id in collection.removed().keys() {
            self.insert((*id).into(), family, true)?;
        }
        Ok(())
    }

    pub(crate) fn evaluated<T: ConstraintType>(
        &mut self,
        family: &'static str,
        collection: &EvaluatedCollection<T>,
    ) -> Result<(), ConstraintNumberCollision> {
        for id in collection.inner().keys() {
            self.insert(
                (*id).into(),
                family,
                collection.removed_reasons().contains_key(id),
            )?;
        }
        Ok(())
    }

    pub(crate) fn sampled<T: ConstraintType>(
        &mut self,
        family: &'static str,
        collection: &SampledCollection<T>,
    ) -> Result<(), ConstraintNumberCollision> {
        for id in collection.inner().keys() {
            self.insert(
                (*id).into(),
                family,
                collection.removed_reasons().contains_key(id),
            )?;
        }
        Ok(())
    }
}
