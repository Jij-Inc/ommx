//! Type family for constraint types.
//!
//! Each constraint type (regular, indicator, etc.) is represented as a marker struct
//! that implements [`ConstraintType`], mapping lifecycle stages to concrete types.
//!
//! This is a defunctionalization of `Stage → Type` since Rust lacks higher-kinded types.

use crate::{
    constraint::{ConstraintID, EvaluatedConstraint, RemovedConstraint, SampledConstraint},
    v1, ATol, Constraint, Evaluate, VariableIDSet,
};
use anyhow::Result;
use std::collections::BTreeMap;

/// A type family for constraints, mapping each lifecycle stage to a concrete type.
///
/// This trait acts as `T: Stage → Type` — a type-level function from lifecycle stages
/// to concrete constraint types. Rust lacks higher-kinded types, so we enumerate
/// the stages as associated types instead.
///
/// Each constraint kind (regular, indicator, etc.) implements this trait via a marker struct.
pub trait ConstraintType {
    /// The constraint as defined in the problem.
    type Created: Evaluate<Output = Self::Evaluated, SampledOutput = Self::Sampled>;
    /// The constraint after being removed/relaxed.
    type Removed: Evaluate<Output = Self::Evaluated, SampledOutput = Self::Sampled>;
    /// The constraint after evaluation against a single state.
    type Evaluated: HasConstraintID;
    /// The constraint after evaluation against multiple samples.
    type Sampled: HasConstraintID;
}

/// Trait for types that carry a ConstraintID.
pub trait HasConstraintID {
    fn constraint_id(&self) -> ConstraintID;
}

impl HasConstraintID for EvaluatedConstraint {
    fn constraint_id(&self) -> ConstraintID {
        self.id
    }
}

impl HasConstraintID for SampledConstraint {
    fn constraint_id(&self) -> ConstraintID {
        self.id
    }
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

/// Marker for regular constraints: `f(x) = 0` or `f(x) <= 0`.
#[derive(Debug, Clone, PartialEq)]
pub struct Regular;

impl ConstraintType for Regular {
    type Created = Constraint;
    type Removed = RemovedConstraint;
    type Evaluated = EvaluatedConstraint;
    type Sampled = SampledConstraint;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, constraint::ConstraintID, linear, Function};

    #[test]
    fn regular_constraint_type_aliases() {
        let c = Constraint::equal_to_zero(ConstraintID::from(1), Function::Zero);
        let _: <Regular as ConstraintType>::Created = c;
    }

    #[test]
    fn empty_collection() {
        let collection = ConstraintCollection::<Regular>::default();
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

        let collection = ConstraintCollection::<Regular>::new(active, BTreeMap::new());

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

        let collection = ConstraintCollection::<Regular>::new(active, BTreeMap::new());
        assert_eq!(collection.active().len(), 1);
        assert_eq!(collection.removed().len(), 0);
    }
}
