mod evaluate;

use crate::{
    constraint::{stage, ConstraintMetadata, Created, Evaluated, Stage},
    constraint_type::{ConstraintType, EvaluatedConstraintBehavior, SampledConstraintBehavior},
    SampleID, VariableID, VariableIDSet,
};
use derive_more::{Deref, From};
use std::collections::{BTreeMap, BTreeSet};

/// ID for one-hot constraints, independent from regular [`ConstraintID`](crate::ConstraintID).
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
pub struct OneHotConstraintID(u64);

impl std::fmt::Debug for OneHotConstraintID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "OneHotConstraintID({})", self.0)
    }
}

impl std::fmt::Display for OneHotConstraintID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl OneHotConstraintID {
    pub fn into_inner(self) -> u64 {
        self.0
    }
}

/// A one-hot constraint: exactly one variable in `variables` must be 1, the rest must be 0.
///
/// This is a structural constraint — no explicit function or equality is stored.
/// The implicit constraint is `sum(x_i) = 1` where all `x_i` are binary.
#[derive(Debug, Clone, PartialEq)]
pub struct OneHotConstraint<S: Stage<Self> = Created> {
    pub id: OneHotConstraintID,
    /// The binary decision variables, exactly one of which must be 1.
    pub variables: BTreeSet<VariableID>,
    pub metadata: ConstraintMetadata,
    pub stage: S::Data,
}

// ===== Stage data =====

/// Data carried by a one-hot constraint in the Created stage.
///
/// One-hot constraints are structural — no function is stored.
#[derive(Debug, Clone, PartialEq)]
pub struct OneHotCreatedData;

/// Data carried by a one-hot constraint in the Evaluated stage.
#[derive(Debug, Clone, PartialEq)]
pub struct OneHotEvaluatedData {
    pub feasible: bool,
    /// Which variable was 1, if exactly one was (None if infeasible).
    pub active_variable: Option<VariableID>,
    pub used_decision_variable_ids: VariableIDSet,
}

/// Data carried by a one-hot constraint in the Sampled stage.
#[derive(Debug, Clone)]
pub struct OneHotSampledData {
    pub feasible: BTreeMap<SampleID, bool>,
    /// Which variable was 1 for each sample.
    pub active_variable: BTreeMap<SampleID, Option<VariableID>>,
    pub used_decision_variable_ids: VariableIDSet,
}

// ===== Stage implementations =====

impl Stage<OneHotConstraint<Created>> for Created {
    type Data = OneHotCreatedData;
}

impl Stage<OneHotConstraint<Evaluated>> for Evaluated {
    type Data = OneHotEvaluatedData;
}

impl Stage<OneHotConstraint<stage::Sampled>> for stage::Sampled {
    type Data = OneHotSampledData;
}

// ===== Type aliases =====

pub type EvaluatedOneHotConstraint = OneHotConstraint<Evaluated>;
pub type SampledOneHotConstraint = OneHotConstraint<stage::Sampled>;

// ===== EvaluatedConstraintBehavior / SampledConstraintBehavior =====

impl EvaluatedConstraintBehavior for EvaluatedOneHotConstraint {
    type ID = OneHotConstraintID;
    fn constraint_id(&self) -> OneHotConstraintID {
        self.id
    }
    fn is_feasible(&self) -> bool {
        self.stage.feasible
    }
}

impl SampledConstraintBehavior for SampledOneHotConstraint {
    type ID = OneHotConstraintID;
    type Evaluated = EvaluatedOneHotConstraint;

    fn constraint_id(&self) -> OneHotConstraintID {
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

        Ok(OneHotConstraint {
            id: self.id,
            variables: self.variables.clone(),
            metadata: self.metadata.clone(),
            stage: OneHotEvaluatedData {
                feasible,
                active_variable,
                used_decision_variable_ids: self.stage.used_decision_variable_ids.clone(),
            },
        })
    }
}

// ===== ConstraintType =====

impl ConstraintType for OneHotConstraint {
    type ID = OneHotConstraintID;
    type Created = OneHotConstraint;
    type Evaluated = EvaluatedOneHotConstraint;
    type Sampled = SampledOneHotConstraint;
}

// ===== Created stage =====

impl OneHotConstraint<Created> {
    /// Create a new one-hot constraint.
    pub fn new(id: OneHotConstraintID, variables: BTreeSet<VariableID>) -> Self {
        Self {
            id,
            variables,
            metadata: ConstraintMetadata::default(),
            stage: OneHotCreatedData,
        }
    }
}

impl std::fmt::Display for OneHotConstraint<Created> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let vars: Vec<String> = self
            .variables
            .iter()
            .map(|v| format!("x{}", v.into_inner()))
            .collect();
        write!(
            f,
            "OneHotConstraint(exactly one of {{{}}} = 1)",
            vars.join(", ")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_one_hot_constraint() {
        let vars: BTreeSet<_> = [1, 2, 3].into_iter().map(VariableID::from).collect();
        let c = OneHotConstraint::new(OneHotConstraintID::from(1), vars.clone());
        assert_eq!(c.id, OneHotConstraintID::from(1));
        assert_eq!(c.variables, vars);
    }

    #[test]
    fn test_display() {
        let vars: BTreeSet<_> = [1, 2, 3].into_iter().map(VariableID::from).collect();
        let c = OneHotConstraint::new(OneHotConstraintID::from(1), vars);
        let s = format!("{}", c);
        assert!(s.contains("OneHotConstraint"));
        assert!(s.contains("x1"));
        assert!(s.contains("x2"));
        assert!(s.contains("x3"));
    }

    #[test]
    fn test_constraint_type_impl() {
        let vars: BTreeSet<_> = [1, 2].into_iter().map(VariableID::from).collect();
        let c = OneHotConstraint::new(OneHotConstraintID::from(1), vars);
        let _: <OneHotConstraint as ConstraintType>::Created = c;
    }
}
