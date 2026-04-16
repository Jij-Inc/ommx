mod evaluate;

use crate::{
    constraint::{stage, ConstraintMetadata, Created, Evaluated, Stage},
    constraint_type::{ConstraintType, EvaluatedConstraintBehavior, SampledConstraintBehavior},
    SampleID, VariableID, VariableIDSet,
};
use derive_more::{Deref, From};
use std::collections::{BTreeMap, BTreeSet};

/// ID for SOS1 constraints, independent from regular [`ConstraintID`](crate::ConstraintID).
#[derive(
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    From,
    Deref,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(transparent)]
pub struct Sos1ConstraintID(u64);

impl std::fmt::Debug for Sos1ConstraintID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Sos1ConstraintID({})", self.0)
    }
}

impl std::fmt::Display for Sos1ConstraintID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Sos1ConstraintID {
    pub fn into_inner(self) -> u64 {
        self.0
    }
}

/// A SOS1 (Special Ordered Set type 1) constraint: at most one variable can be non-zero.
///
/// This is a structural constraint — no explicit function or equality is stored.
/// Unlike [`OneHotConstraint`](crate::OneHotConstraint), SOS1 allows all variables to be zero.
#[derive(Debug, Clone, PartialEq)]
pub struct Sos1Constraint<S: Stage<Self> = Created> {
    pub id: Sos1ConstraintID,
    /// The decision variables, at most one of which can be non-zero.
    pub variables: BTreeSet<VariableID>,
    pub metadata: ConstraintMetadata,
    pub stage: S::Data,
}

// ===== Stage data =====

/// Data carried by a SOS1 constraint in the Created stage.
///
/// SOS1 constraints are structural — no function is stored.
#[derive(Debug, Clone, PartialEq)]
pub struct Sos1CreatedData;

/// Data carried by a SOS1 constraint in the Evaluated stage.
#[derive(Debug, Clone, PartialEq)]
pub struct Sos1EvaluatedData {
    pub feasible: bool,
    /// Which variable was non-zero, if exactly one was (None if all zero or infeasible).
    pub active_variable: Option<VariableID>,
    pub used_decision_variable_ids: VariableIDSet,
}

/// Data carried by a SOS1 constraint in the Sampled stage.
#[derive(Debug, Clone)]
pub struct Sos1SampledData {
    pub feasible: BTreeMap<SampleID, bool>,
    /// Which variable was non-zero for each sample.
    pub active_variable: BTreeMap<SampleID, Option<VariableID>>,
    pub used_decision_variable_ids: VariableIDSet,
}

// ===== Stage implementations =====

impl Stage<Sos1Constraint<Created>> for Created {
    type Data = Sos1CreatedData;
}

impl Stage<Sos1Constraint<Evaluated>> for Evaluated {
    type Data = Sos1EvaluatedData;
}

impl Stage<Sos1Constraint<stage::Sampled>> for stage::Sampled {
    type Data = Sos1SampledData;
}

// ===== Type aliases =====

pub type EvaluatedSos1Constraint = Sos1Constraint<Evaluated>;
pub type SampledSos1Constraint = Sos1Constraint<stage::Sampled>;

// ===== EvaluatedConstraintBehavior / SampledConstraintBehavior =====

impl EvaluatedConstraintBehavior for EvaluatedSos1Constraint {
    type ID = Sos1ConstraintID;
    fn constraint_id(&self) -> Sos1ConstraintID {
        self.id
    }
    fn is_feasible(&self) -> bool {
        self.stage.feasible
    }
}

impl SampledConstraintBehavior for SampledSos1Constraint {
    type ID = Sos1ConstraintID;
    type Evaluated = EvaluatedSos1Constraint;

    fn constraint_id(&self) -> Sos1ConstraintID {
        self.id
    }
    fn is_feasible_for(&self, sample_id: SampleID) -> Option<bool> {
        self.stage.feasible.get(&sample_id).copied()
    }
    fn get(
        &self,
        sample_id: SampleID,
    ) -> Result<Self::Evaluated, crate::sampled::UnknownSampleIDError> {
        let feasible = *self
            .stage
            .feasible
            .get(&sample_id)
            .ok_or(crate::sampled::UnknownSampleIDError { id: sample_id })?;
        let active_variable = *self
            .stage
            .active_variable
            .get(&sample_id)
            .ok_or(crate::sampled::UnknownSampleIDError { id: sample_id })?;

        Ok(Sos1Constraint {
            id: self.id,
            variables: self.variables.clone(),
            metadata: self.metadata.clone(),
            stage: Sos1EvaluatedData {
                feasible,
                active_variable,
                used_decision_variable_ids: self.stage.used_decision_variable_ids.clone(),
            },
        })
    }
}

// ===== ConstraintType =====

impl ConstraintType for Sos1Constraint {
    type ID = Sos1ConstraintID;
    type Created = Sos1Constraint;
    type Evaluated = EvaluatedSos1Constraint;
    type Sampled = SampledSos1Constraint;
}

// ===== Created stage =====

impl Sos1Constraint<Created> {
    /// Create a new SOS1 constraint.
    pub fn new(id: Sos1ConstraintID, variables: BTreeSet<VariableID>) -> Self {
        Self {
            id,
            variables,
            metadata: ConstraintMetadata::default(),
            stage: Sos1CreatedData,
        }
    }
}

impl std::fmt::Display for Sos1Constraint<Created> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let vars: Vec<String> = self
            .variables
            .iter()
            .map(|v| format!("x{}", v.into_inner()))
            .collect();
        write!(
            f,
            "Sos1Constraint(at most one of {{{}}} ≠ 0)",
            vars.join(", ")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_sos1_constraint() {
        let vars: BTreeSet<_> = [1, 2, 3].into_iter().map(VariableID::from).collect();
        let c = Sos1Constraint::new(Sos1ConstraintID::from(1), vars.clone());
        assert_eq!(c.id, Sos1ConstraintID::from(1));
        assert_eq!(c.variables, vars);
    }

    #[test]
    fn test_display() {
        let vars: BTreeSet<_> = [1, 2, 3].into_iter().map(VariableID::from).collect();
        let c = Sos1Constraint::new(Sos1ConstraintID::from(1), vars);
        let s = format!("{}", c);
        assert!(s.contains("Sos1Constraint"));
        assert!(s.contains("x1"));
    }

    #[test]
    fn test_constraint_type_impl() {
        let vars: BTreeSet<_> = [1, 2].into_iter().map(VariableID::from).collect();
        let c = Sos1Constraint::new(Sos1ConstraintID::from(1), vars);
        let _: <Sos1Constraint as ConstraintType>::Created = c;
    }
}
