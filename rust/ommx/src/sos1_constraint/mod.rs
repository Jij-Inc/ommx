mod evaluate;

use crate::{
    constraint::{stage, Created, Evaluated, Stage},
    constraint_type::{
        sample_ids_from_map, ConstraintType, EvaluatedConstraintBehavior, SampledConstraintBehavior,
    },
    Parse, ParseError, SampleID, SampleIDSet, VariableID, VariableIDSet,
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
    crate::logical_memory::LogicalMemoryProfile,
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

impl From<Sos1ConstraintID> for u64 {
    fn from(id: Sos1ConstraintID) -> Self {
        id.0
    }
}

/// A SOS1 (Special Ordered Set type 1) constraint: at most one variable can be non-zero.
///
/// This is a structural constraint — no explicit function or equality is stored.
/// Unlike [`OneHotConstraint`](crate::OneHotConstraint), SOS1 allows all variables to be zero.
///
/// The constraint's [`Sos1ConstraintID`] is not stored in this struct — it is held
/// by the enclosing collection. Modeling labels and provenance live on the
/// enclosing collection's [`ConstraintContextStore`](crate::ConstraintContextStore).
///
/// [`Instance`]: crate::Instance
#[derive(Debug, Clone, PartialEq)]
pub struct Sos1Constraint<S: Stage<Self> = Created> {
    /// The decision variables, at most one of which can be non-zero.
    pub variables: BTreeSet<VariableID>,
    pub stage: S::Data,
}

// ===== Stage data =====

/// Data carried by a SOS1 constraint in the Created stage.
///
/// SOS1 constraints are structural — no function is stored.
#[derive(Debug, Clone, PartialEq, crate::logical_memory::LogicalMemoryProfile)]
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
    fn is_feasible(&self) -> bool {
        self.stage.feasible
    }

    fn used_decision_variable_ids(&self) -> &VariableIDSet {
        &self.stage.used_decision_variable_ids
    }
}

impl SampledConstraintBehavior for SampledSos1Constraint {
    type ID = Sos1ConstraintID;
    type Evaluated = EvaluatedSos1Constraint;

    fn is_feasible_for(&self, sample_id: SampleID) -> Option<bool> {
        self.stage.feasible.get(&sample_id).copied()
    }

    fn validate_sample_ids(&self, expected: &SampleIDSet) -> std::result::Result<(), SampleIDSet> {
        let feasible_ids = sample_ids_from_map(&self.stage.feasible);
        if &feasible_ids != expected {
            return Err(feasible_ids);
        }
        let active_variable_ids = sample_ids_from_map(&self.stage.active_variable);
        if &active_variable_ids != expected {
            return Err(active_variable_ids);
        }
        Ok(())
    }

    fn used_decision_variable_ids(&self) -> &VariableIDSet {
        &self.stage.used_decision_variable_ids
    }

    fn get(&self, sample_id: SampleID) -> Option<Self::Evaluated> {
        let feasible = *self.stage.feasible.get(&sample_id)?;
        let active_variable = *self.stage.active_variable.get(&sample_id)?;

        Some(Sos1Constraint {
            variables: self.variables.clone(),
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
    ///
    /// # Errors
    ///
    /// Returns an error if `variables` is empty.
    pub fn new(variables: BTreeSet<VariableID>) -> crate::Result<Self> {
        if variables.is_empty() {
            crate::bail!("SOS1 constraints must contain at least one variable");
        }
        Ok(Self {
            variables,
            stage: Sos1CreatedData,
        })
    }
}

impl From<Sos1Constraint<Created>> for crate::v2::Sos1Constraint {
    fn from(constraint: Sos1Constraint<Created>) -> Self {
        Self {
            variables: constraint
                .variables
                .into_iter()
                .map(|id| id.into_inner())
                .collect(),
        }
    }
}

impl Parse for crate::v2::Sos1Constraint {
    type Output = Sos1Constraint<Created>;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v2.Sos1Constraint";
        Sos1Constraint::new(crate::v2_io::variable_id_set_from_v2(
            self.variables,
            message,
            "variables",
        )?)
        .map_err(|e| {
            crate::RawParseError::InvalidInstance(e.to_string()).context(message, "variables")
        })
    }
}

impl From<EvaluatedSos1Constraint> for crate::v2::EvaluatedSos1Constraint {
    fn from(constraint: EvaluatedSos1Constraint) -> Self {
        Self {
            variables: constraint
                .variables
                .into_iter()
                .map(|id| id.into_inner())
                .collect(),
            feasible: constraint.stage.feasible,
            active_variable: constraint.stage.active_variable.map(|id| id.into_inner()),
            used_decision_variable_ids: constraint
                .stage
                .used_decision_variable_ids
                .into_iter()
                .map(|id| id.into_inner())
                .collect(),
        }
    }
}

impl Parse for crate::v2::EvaluatedSos1Constraint {
    type Output = EvaluatedSos1Constraint;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v2.EvaluatedSos1Constraint";
        let variables =
            crate::v2_io::variable_id_set_from_v2(self.variables, message, "variables")?;
        if variables.is_empty() {
            return Err(crate::RawParseError::InvalidInstance(
                "SOS1 constraints must contain at least one variable".to_string(),
            )
            .context(message, "variables"));
        }
        let active_variable = self.active_variable.map(VariableID::from);
        if active_variable.is_some_and(|id| !variables.contains(&id)) {
            return Err(crate::RawParseError::InvalidInstance(
                "SOS1 active_variable must be a member of variables".to_string(),
            )
            .context(message, "active_variable"));
        }
        Ok(Sos1Constraint {
            variables,
            stage: Sos1EvaluatedData {
                feasible: self.feasible,
                active_variable,
                used_decision_variable_ids: crate::v2_io::variable_id_set_from_v2(
                    self.used_decision_variable_ids,
                    message,
                    "used_decision_variable_ids",
                )?,
            },
        })
    }
}

impl From<SampledSos1Constraint> for crate::v2::SampledSos1Constraint {
    fn from(constraint: SampledSos1Constraint) -> Self {
        Self {
            variables: constraint
                .variables
                .into_iter()
                .map(|id| id.into_inner())
                .collect(),
            feasible: constraint
                .stage
                .feasible
                .into_iter()
                .map(|(id, value)| (id.into_inner(), value))
                .collect(),
            active_variable: constraint
                .stage
                .active_variable
                .into_iter()
                .map(|(id, variable_id)| {
                    (
                        id.into_inner(),
                        crate::v2::SampledActiveVariable {
                            variable_id: variable_id.map(|id| id.into_inner()),
                        },
                    )
                })
                .collect(),
            used_decision_variable_ids: constraint
                .stage
                .used_decision_variable_ids
                .into_iter()
                .map(|id| id.into_inner())
                .collect(),
        }
    }
}

impl Parse for crate::v2::SampledSos1Constraint {
    type Output = SampledSos1Constraint;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v2.SampledSos1Constraint";
        let variables =
            crate::v2_io::variable_id_set_from_v2(self.variables, message, "variables")?;
        if variables.is_empty() {
            return Err(crate::RawParseError::InvalidInstance(
                "SOS1 constraints must contain at least one variable".to_string(),
            )
            .context(message, "variables"));
        }
        let active_variable =
            crate::v2_io::sampled_active_variable_map_from_v2(self.active_variable);
        if active_variable
            .values()
            .flatten()
            .any(|id| !variables.contains(id))
        {
            return Err(crate::RawParseError::InvalidInstance(
                "SOS1 active_variable values must be members of variables".to_string(),
            )
            .context(message, "active_variable"));
        }
        Ok(Sos1Constraint {
            variables,
            stage: Sos1SampledData {
                feasible: crate::v2_io::sample_bool_map_from_v2(self.feasible),
                active_variable,
                used_decision_variable_ids: crate::v2_io::variable_id_set_from_v2(
                    self.used_decision_variable_ids,
                    message,
                    "used_decision_variable_ids",
                )?,
            },
        })
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
        let c = Sos1Constraint::new(vars.clone()).unwrap();
        assert_eq!(c.variables, vars);
    }

    #[test]
    fn sos1_constraint_rejects_empty_variable_set() {
        let err = Sos1Constraint::new(BTreeSet::new()).unwrap_err();
        assert!(err.to_string().contains("at least one variable"));
    }

    #[test]
    fn test_display() {
        let vars: BTreeSet<_> = [1, 2, 3].into_iter().map(VariableID::from).collect();
        let c = Sos1Constraint::new(vars).unwrap();
        let s = format!("{}", c);
        assert!(s.contains("Sos1Constraint"));
        assert!(s.contains("x1"));
    }

    #[test]
    fn test_constraint_type_impl() {
        let vars: BTreeSet<_> = [1, 2].into_iter().map(VariableID::from).collect();
        let c = Sos1Constraint::new(vars).unwrap();
        let _: <Sos1Constraint as ConstraintType>::Created = c;
    }
}
