mod evaluate;

use crate::{
    constraint::{stage, Created, Evaluated, Stage},
    constraint_type::{
        sample_ids_from_map, ConstraintType, EvaluatedConstraintBehavior, EvaluatedConstraintData,
        SampledConstraintBehavior, SampledConstraintData,
    },
    ATol, Parse, ParseError, SampleID, SampleIDSet, VariableID, VariableIDSet,
};
use derive_more::{Deref, From};
use std::collections::{BTreeMap, BTreeSet};

/// Validation failures for SOS1 constraints.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Sos1ConstraintError {
    /// A SOS1 constraint has no variables to constrain.
    #[error("SOS1 constraints must contain at least one variable")]
    EmptyVariables,
}

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
///
/// `violation` must be nonnegative and not NaN. Feasibility is derived from
/// `violation` and the query tolerance. Evaluation computes these values; Solution validates
/// the metric and active-variable diagnostic against its decision-variable values.
#[derive(Debug, Clone, PartialEq)]
pub struct Sos1EvaluatedData {
    /// Tolerance used to determine the retained activation diagnostic.
    /// Feasibility queries supply their own tolerance and do not change it.
    pub activation_atol: ATol,
    pub violation: f64,
    /// Retained member at `activation_atol`; None when infeasible at that
    /// tolerance or all members are approximately zero under it.
    pub active_variable: Option<VariableID>,
    pub used_decision_variable_ids: VariableIDSet,
}

/// Data carried by a SOS1 constraint in the Sampled stage.
///
/// Each violation is nonnegative and not NaN. The violation and active-variable
/// maps must have the same sample IDs. Evaluation constructs this data and
/// SampleSet validates the metrics and diagnostics against its sampled variables.
#[derive(Debug, Clone)]
pub struct Sos1SampledData {
    /// Tolerance used to determine the retained activation diagnostic.
    /// Feasibility queries supply their own tolerance and do not change it.
    pub activation_atol: ATol,
    pub violations: crate::Sampled<f64>,
    /// Retained member for each sample, with the same meaning as the evaluated stage.
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

impl EvaluatedConstraintData for EvaluatedSos1Constraint {
    type ID = Sos1ConstraintID;
    fn violation(&self) -> f64 {
        self.stage.violation
    }

    fn used_decision_variable_ids(&self) -> &VariableIDSet {
        &self.stage.used_decision_variable_ids
    }
}

impl SampledConstraintData for SampledSos1Constraint {
    type ID = Sos1ConstraintID;
    type Evaluated = EvaluatedSos1Constraint;

    fn violation_for(&self, sample_id: SampleID) -> Option<f64> {
        self.stage.violations.get(sample_id).copied()
    }

    fn validate_sample_ids(&self, expected: &SampleIDSet) -> std::result::Result<(), SampleIDSet> {
        if !self.stage.violations.has_same_ids(expected) {
            return Err(self.stage.violations.ids());
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
        let violation = *self.stage.violations.get(sample_id)?;
        let active_variable = *self.stage.active_variable.get(&sample_id)?;

        Some(Sos1Constraint {
            variables: self.variables.clone(),
            stage: Sos1EvaluatedData {
                activation_atol: self.stage.activation_atol,
                violation,
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
    /// The error chain contains [`Sos1ConstraintError::EmptyVariables`] if
    /// `variables` is empty.
    pub fn new(variables: BTreeSet<VariableID>) -> crate::Result<Self> {
        crate::ensure!(!variables.is_empty(), Sos1ConstraintError::EmptyVariables);
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
        .map_err(|error| ParseError::new(error).context(message, "variables"))
    }
}

impl EvaluatedSos1Constraint {
    /// Solution/SampleSet supplies the tolerance for the wire feasibility field.
    pub(crate) fn into_v2(self, atol: ATol) -> crate::v2::EvaluatedSos1Constraint {
        let constraint = self;
        let feasible = constraint.is_feasible(atol);
        crate::v2::EvaluatedSos1Constraint {
            variables: constraint
                .variables
                .into_iter()
                .map(|id| id.into_inner())
                .collect(),
            feasible,
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

impl crate::v2::EvaluatedSos1Constraint {
    /// Restore the SDK metric from values supplied by the enclosing Solution or
    /// SampleSet. Structural wire rows contain member IDs, not their values.
    pub(crate) fn parse_with_values(
        self,
        value_of: impl FnMut(VariableID) -> Option<f64>,
        atol: ATol,
    ) -> Result<EvaluatedSos1Constraint, ParseError> {
        let message = "ommx.v2.EvaluatedSos1Constraint";
        let variables =
            crate::v2_io::variable_id_set_from_v2(self.variables, message, "variables")?;
        if variables.is_empty() {
            return Err(
                ParseError::new(Sos1ConstraintError::EmptyVariables).context(message, "variables")
            );
        }
        let active_variable = self.active_variable.map(VariableID::from);
        if active_variable.is_some_and(|id| !variables.contains(&id)) {
            return Err(ParseError::new(crate::error!(
                "SOS1 active_variable must be a member of variables"
            ))
            .context(message, "active_variable"));
        }
        if active_variable.is_some() && !self.feasible {
            return Err(ParseError::new(crate::error!(
                "SOS1 active_variable must be unset when feasible is false"
            ))
            .context(message, "active_variable"));
        }
        let constraint = Sos1Constraint::new(variables)
            .map_err(|error| ParseError::new(error).context(message, "variables"))?;
        let (violation, _) = constraint
            .evaluate_members(value_of, atol)
            .map_err(|error| ParseError::new(error).context(message, "variables"))?;
        crate::constraint_type::validate_wire_feasibility(violation, self.feasible, atol, message)?;
        Ok(Sos1Constraint {
            variables: constraint.variables,
            stage: Sos1EvaluatedData {
                activation_atol: atol,
                violation,
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

impl SampledSos1Constraint {
    /// Solution/SampleSet supplies the tolerance for the wire feasibility field.
    pub(crate) fn into_v2(self, atol: ATol) -> crate::v2::SampledSos1Constraint {
        let constraint = self;
        let feasible = constraint
            .stage
            .violations
            .iter()
            .map(|(id, _)| {
                (
                    id.into_inner(),
                    constraint
                        .is_feasible_for(*id, atol)
                        .expect("sample exists"),
                )
            })
            .collect();
        crate::v2::SampledSos1Constraint {
            variables: constraint
                .variables
                .into_iter()
                .map(|id| id.into_inner())
                .collect(),
            feasible,
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

impl crate::v2::SampledSos1Constraint {
    /// Restore the SDK metric from values supplied by the enclosing Solution or
    /// SampleSet. Structural wire rows contain member IDs, not their values.
    pub(crate) fn parse_with_values(
        self,
        mut value_of: impl FnMut(SampleID, VariableID) -> Option<f64>,
        atol: ATol,
    ) -> Result<SampledSos1Constraint, ParseError> {
        let message = "ommx.v2.SampledSos1Constraint";
        let variables =
            crate::v2_io::variable_id_set_from_v2(self.variables, message, "variables")?;
        if variables.is_empty() {
            return Err(
                ParseError::new(Sos1ConstraintError::EmptyVariables).context(message, "variables")
            );
        }
        let active_variable =
            crate::v2_io::sampled_active_variable_map_from_v2(self.active_variable);
        if active_variable
            .values()
            .flatten()
            .any(|id| !variables.contains(id))
        {
            return Err(ParseError::new(crate::error!(
                "SOS1 active_variable values must be members of variables"
            ))
            .context(message, "active_variable"));
        }
        let feasible = crate::v2_io::sample_bool_map_from_v2(self.feasible);
        if sample_ids_from_map(&feasible) != sample_ids_from_map(&active_variable) {
            return Err(ParseError::new(crate::error!(
                "feasible and active_variable sample IDs must match"
            ))
            .context(message, "feasible"));
        }
        for id in feasible.keys() {
            if active_variable[id].is_some() && !feasible[id] {
                return Err(ParseError::new(crate::error!(
                    "SOS1 active_variable must be unset when feasible is false"
                ))
                .context(message, "active_variable"));
            }
        }
        let constraint = Sos1Constraint::new(variables)
            .map_err(|error| ParseError::new(error).context(message, "variables"))?;
        let mut violations = crate::Sampled::default();
        for (&id, &provided_feasible) in &feasible {
            let (violation, _) = constraint
                .evaluate_members(|variable_id| value_of(id, variable_id), atol)
                .map_err(|error| ParseError::new(error).context(message, "variables"))?;
            crate::constraint_type::validate_wire_feasibility(
                violation,
                provided_feasible,
                atol,
                message,
            )?;
            violations
                .append([id], violation)
                .map_err(|error| ParseError::new(error).context(message, "feasible"))?;
        }
        Ok(Sos1Constraint {
            variables: constraint.variables,
            stage: Sos1SampledData {
                activation_atol: atol,
                violations,
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

impl<S: Stage<Self>> Sos1Constraint<S> {
    /// Evaluate structural member values supplied by a state or a validated host.
    /// The constraint owns both its metric and the active-variable diagnostic;
    /// Solution and SampleSet use this same operation when validating restored rows.
    pub(crate) fn evaluate_members(
        &self,
        mut value_of: impl FnMut(VariableID) -> Option<f64>,
        atol: ATol,
    ) -> crate::Result<(f64, Option<VariableID>)> {
        let mut values = Vec::with_capacity(self.variables.len());
        for &id in &self.variables {
            let value = value_of(id).ok_or_else(|| {
                crate::error!("Variable {id:?} not found in state for SOS1 constraint")
            })?;
            crate::ensure!(
                value.is_finite(),
                "Variable {id:?} in SOS1 constraint must be finite (value={value})"
            );
            values.push((id, value));
        }
        let &(selected, selected_value) = values
            .iter()
            .max_by(|(_, a), (_, b)| a.abs().total_cmp(&b.abs()))
            .ok_or_else(|| crate::error!("SOS1 constraints must contain at least one variable"))?;
        // Sum nonnegative terms directly: sum-minus-maximum can erase small violations.
        let violation = values
            .iter()
            .map(
                |&(id, value)| {
                    if id == selected {
                        0.0
                    } else {
                        value.abs()
                    }
                },
            )
            .fold(0.0, |total, value| total + value);
        let active_variable = if crate::constraint_type::violation_is_feasible(violation, atol) {
            (!atol.approx_is_zero(selected_value)).then_some(selected)
        } else {
            None
        };
        Ok((violation, active_variable))
    }
}

impl EvaluatedSos1Constraint {
    /// Minimum sum of absolute member changes needed to satisfy this constraint.
    pub fn violation(&self) -> f64 {
        EvaluatedConstraintData::violation(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error as _;

    fn error_chain_contains<T: std::error::Error + 'static>(
        error: &(dyn std::error::Error + 'static),
    ) -> bool {
        error.downcast_ref::<T>().is_some() || error.source().is_some_and(error_chain_contains::<T>)
    }

    #[test]
    fn test_create_sos1_constraint() {
        let vars: BTreeSet<_> = [1, 2, 3].into_iter().map(VariableID::from).collect();
        let c = Sos1Constraint::new(vars.clone()).unwrap();
        assert_eq!(c.variables, vars);
    }

    #[test]
    fn sos1_constraint_rejects_empty_variable_set() {
        let err = Sos1Constraint::new(BTreeSet::new()).unwrap_err();
        assert!(matches!(
            err.downcast_ref::<Sos1ConstraintError>(),
            Some(Sos1ConstraintError::EmptyVariables)
        ));
    }

    #[test]
    fn parse_v2_preserves_empty_variables_signal() {
        let parse_error = crate::v2::Sos1Constraint { variables: vec![] }
            .parse(&())
            .unwrap_err();

        assert!(matches!(
            parse_error
                .source()
                .and_then(|error| error.downcast_ref::<Sos1ConstraintError>()),
            Some(Sos1ConstraintError::EmptyVariables)
        ));
    }

    #[test]
    fn parse_v2_evaluated_preserves_empty_variables_signal() {
        let parse_error = crate::v2::EvaluatedSos1Constraint::default()
            .parse_with_values(|_| None, ATol::default())
            .unwrap_err();

        assert!(matches!(
            parse_error
                .source()
                .and_then(|error| error.downcast_ref::<Sos1ConstraintError>()),
            Some(Sos1ConstraintError::EmptyVariables)
        ));
    }

    #[test]
    fn parse_v2_sampled_preserves_empty_variables_signal() {
        let parse_error = crate::v2::SampledSos1Constraint::default()
            .parse_with_values(|_, _| None, ATol::default())
            .unwrap_err();

        assert!(matches!(
            parse_error
                .source()
                .and_then(|error| error.downcast_ref::<Sos1ConstraintError>()),
            Some(Sos1ConstraintError::EmptyVariables)
        ));
    }

    #[test]
    fn parse_v2_evaluated_rejects_infeasible_active_variable() {
        let proto = crate::v2::EvaluatedSos1Constraint {
            variables: vec![1],
            feasible: false,
            active_variable: Some(1),
            used_decision_variable_ids: vec![],
        };

        let err = proto
            .parse_with_values(|_| None, ATol::default())
            .unwrap_err();

        assert!(!error_chain_contains::<crate::RawParseError>(&err));
        assert!(!error_chain_contains::<Sos1ConstraintError>(&err));
        assert!(
            err.to_string()
                .contains("active_variable must be unset when feasible is false"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_v2_sampled_rejects_infeasible_active_variable() {
        let proto = crate::v2::SampledSos1Constraint {
            variables: vec![1],
            feasible: BTreeMap::from([(0, false)]),
            active_variable: BTreeMap::from([(
                0,
                crate::v2::SampledActiveVariable {
                    variable_id: Some(1),
                },
            )]),
            used_decision_variable_ids: vec![],
        };

        let err = proto
            .parse_with_values(|_, _| None, ATol::default())
            .unwrap_err();

        assert!(!error_chain_contains::<crate::RawParseError>(&err));
        assert!(!error_chain_contains::<Sos1ConstraintError>(&err));
        assert!(
            err.to_string()
                .contains("active_variable must be unset when feasible is false"),
            "unexpected error: {err}"
        );
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
