mod diagnostics;
pub use diagnostics::OneHotViolationDiagnostics;
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

/// Shared by OneHot evaluation, propagation, wire restoration, and SampleSet
/// validation (including rows with no samples). The restriction belongs to
/// OneHot's discrete interpretation, not to the general ATol scalar.
pub(crate) fn validate_atol(atol: ATol) -> crate::Result<()> {
    crate::ensure!(
        atol < 0.5,
        "OneHot evaluation and propagation require atol < 0.5: got {}",
        atol.into_inner()
    );
    Ok(())
}

/// Validation failures for one-hot constraints.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum OneHotConstraintError {
    /// A one-hot constraint has no variables to select from.
    #[error("One-hot constraints must contain at least one variable")]
    EmptyVariables,
}

/// Family-specific ID for one-hot constraints.
/// Its numeric value is unique across every constraint family in an instance,
/// including inactive constraints.
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

impl From<OneHotConstraintID> for u64 {
    fn from(id: OneHotConstraintID) -> Self {
        id.0
    }
}

/// A one-hot constraint: exactly one variable in `variables` must be 1, the rest must be 0.
///
/// This is a structural constraint — no explicit function or equality is stored.
/// The implicit constraint is `sum(x_i) = 1` where all `x_i` are binary.
///
/// Evaluation and propagation require `0 < atol < 0.5`, so the tolerated
/// neighborhoods of zero and one do not overlap. This also applies when
/// restoring evaluated or sampled OneHot rows from stored result values.
///
/// The constraint's [`OneHotConstraintID`] is not stored in this struct — it is held
/// by the enclosing collection. Modeling labels and provenance live on the
/// enclosing collection's [`ConstraintContextStore`](crate::ConstraintContextStore).
///
/// [`Instance`]: crate::Instance
#[derive(Debug, Clone, PartialEq)]
pub struct OneHotConstraint<S: Stage<Self> = Created> {
    /// The binary decision variables, exactly one of which must be 1.
    pub variables: BTreeSet<VariableID>,
    pub stage: S::Data,
}

// ===== Stage data =====

/// Data carried by a one-hot constraint in the Created stage.
///
/// One-hot constraints are structural — no function is stored.
#[derive(Debug, Clone, PartialEq, crate::logical_memory::LogicalMemoryProfile)]
pub struct OneHotCreatedData;

/// Data carried by a one-hot constraint in the Evaluated stage.
///
/// `violation` must be nonnegative and not NaN. Feasibility is derived from
/// `violation` and the query tolerance. Evaluation computes these values; Solution validates
/// the metric and selected-variable diagnostic against its decision-variable values.
#[derive(Debug, Clone, PartialEq)]
pub struct OneHotEvaluatedData {
    /// Tolerance used to determine the retained activation diagnostic.
    /// Feasibility queries supply their own tolerance and do not change it.
    pub activation_atol: ATol,
    pub violation: f64,
    /// Selected variable at `activation_atol`; None if infeasible at that tolerance.
    pub selected_variable: Option<VariableID>,
    pub used_decision_variable_ids: VariableIDSet,
}

/// Data carried by a one-hot constraint in the Sampled stage.
///
/// Each violation is nonnegative and not NaN. The violation and selected-variable
/// maps must have the same sample IDs. Evaluation constructs this data and
/// SampleSet validates the metrics and diagnostics against its sampled variables.
#[derive(Debug, Clone, PartialEq)]
pub struct OneHotSampledData {
    /// Tolerance used to determine the retained activation diagnostic.
    /// Feasibility queries supply their own tolerance and do not change it.
    pub activation_atol: ATol,
    pub violations: crate::Sampled<f64>,
    /// Selected variable at `activation_atol`; None if infeasible at that tolerance.
    pub selected_variable: BTreeMap<SampleID, Option<VariableID>>,
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

impl EvaluatedConstraintData for EvaluatedOneHotConstraint {
    type ID = OneHotConstraintID;
    fn violation(&self) -> f64 {
        self.stage.violation
    }

    fn used_decision_variable_ids(&self) -> &VariableIDSet {
        &self.stage.used_decision_variable_ids
    }
}

impl SampledConstraintData for SampledOneHotConstraint {
    type ID = OneHotConstraintID;
    type Evaluated = EvaluatedOneHotConstraint;
    fn violation_for(&self, sample_id: SampleID) -> Option<f64> {
        self.stage.violations.get(sample_id).copied()
    }

    fn validate_sample_ids(&self, expected: &SampleIDSet) -> std::result::Result<(), SampleIDSet> {
        if !self.stage.violations.has_same_ids(expected) {
            return Err(self.stage.violations.ids());
        }
        let selected_variable_ids = sample_ids_from_map(&self.stage.selected_variable);
        if &selected_variable_ids != expected {
            return Err(selected_variable_ids);
        }
        Ok(())
    }

    fn used_decision_variable_ids(&self) -> &VariableIDSet {
        &self.stage.used_decision_variable_ids
    }

    fn get(&self, sample_id: SampleID) -> Option<Self::Evaluated> {
        let violation = *self.stage.violations.get(sample_id)?;
        let selected_variable = *self.stage.selected_variable.get(&sample_id)?;

        Some(OneHotConstraint {
            variables: self.variables.clone(),
            stage: OneHotEvaluatedData {
                activation_atol: self.stage.activation_atol,
                violation,
                selected_variable,
                used_decision_variable_ids: self.stage.used_decision_variable_ids.clone(),
            },
        })
    }
}

// ===== ConstraintType =====

impl ConstraintType for OneHotConstraint {
    type ID = OneHotConstraintID;
    type ViolationDiagnostics = crate::OneHotViolationDiagnostics;
    type Created = OneHotConstraint;
    type Evaluated = EvaluatedOneHotConstraint;
    type Sampled = SampledOneHotConstraint;
}

// ===== Created stage =====

impl OneHotConstraint<Created> {
    /// Create a new one-hot constraint.
    ///
    /// # Errors
    ///
    /// The error chain contains [`OneHotConstraintError::EmptyVariables`] if
    /// `variables` is empty.
    pub fn new(variables: BTreeSet<VariableID>) -> crate::Result<Self> {
        crate::ensure!(!variables.is_empty(), OneHotConstraintError::EmptyVariables);
        Ok(Self {
            variables,
            stage: OneHotCreatedData,
        })
    }
}

impl From<OneHotConstraint<Created>> for crate::v2::OneHotConstraint {
    fn from(constraint: OneHotConstraint<Created>) -> Self {
        Self {
            variables: constraint
                .variables
                .into_iter()
                .map(|id| id.into_inner())
                .collect(),
        }
    }
}

impl Parse for crate::v2::OneHotConstraint {
    type Output = OneHotConstraint<Created>;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v2.OneHotConstraint";
        OneHotConstraint::new(crate::v2_io::variable_id_set_from_v2(
            self.variables,
            message,
            "variables",
        )?)
        .map_err(|error| ParseError::new(error).context(message, "variables"))
    }
}

impl EvaluatedOneHotConstraint {
    /// Solution/SampleSet supplies the tolerance for the wire feasibility field.
    pub(crate) fn into_v2(self, atol: ATol) -> crate::v2::EvaluatedOneHotConstraint {
        let constraint = self;
        let feasible = constraint.is_feasible(atol);
        crate::v2::EvaluatedOneHotConstraint {
            variables: constraint
                .variables
                .into_iter()
                .map(|id| id.into_inner())
                .collect(),
            feasible,
            selected_variable: constraint.stage.selected_variable.map(|id| id.into_inner()),
            used_decision_variable_ids: constraint
                .stage
                .used_decision_variable_ids
                .into_iter()
                .map(|id| id.into_inner())
                .collect(),
        }
    }
}

impl crate::v2::EvaluatedOneHotConstraint {
    /// Restore the SDK metric from values supplied by the enclosing Solution or
    /// SampleSet. Structural wire rows contain member IDs, not their values.
    pub(crate) fn parse_with_values(
        self,
        value_of: impl FnMut(VariableID) -> Option<f64>,
        atol: ATol,
    ) -> Result<EvaluatedOneHotConstraint, ParseError> {
        validate_atol(atol).map_err(ParseError::new)?;
        let message = "ommx.v2.EvaluatedOneHotConstraint";
        let variables =
            crate::v2_io::variable_id_set_from_v2(self.variables, message, "variables")?;
        if variables.is_empty() {
            return Err(ParseError::new(OneHotConstraintError::EmptyVariables)
                .context(message, "variables"));
        }
        let selected_variable = self.selected_variable.map(VariableID::from);
        if selected_variable.is_some_and(|id| !variables.contains(&id)) {
            return Err(ParseError::new(crate::error!(
                "One-hot selected_variable must be a member of variables"
            ))
            .context(message, "selected_variable"));
        }
        if self.feasible != selected_variable.is_some() {
            return Err(ParseError::new(crate::error!(
                "One-hot feasible must be true exactly when selected_variable is set"
            ))
            .context(message, "selected_variable"));
        }
        let constraint = OneHotConstraint::new(variables)
            .map_err(|error| ParseError::new(error).context(message, "variables"))?;
        let (violation, _) = constraint
            .evaluate_members(value_of, atol)
            .map_err(|error| ParseError::new(error).context(message, "variables"))?;
        crate::constraint_type::validate_wire_feasibility(violation, self.feasible, atol, message)?;
        Ok(OneHotConstraint {
            variables: constraint.variables,
            stage: OneHotEvaluatedData {
                activation_atol: atol,
                violation,
                selected_variable,
                used_decision_variable_ids: crate::v2_io::variable_id_set_from_v2(
                    self.used_decision_variable_ids,
                    message,
                    "used_decision_variable_ids",
                )?,
            },
        })
    }
}

impl SampledOneHotConstraint {
    /// Solution/SampleSet supplies the tolerance for the wire feasibility field.
    pub(crate) fn into_v2(self, atol: ATol) -> crate::v2::SampledOneHotConstraint {
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
        crate::v2::SampledOneHotConstraint {
            variables: constraint
                .variables
                .into_iter()
                .map(|id| id.into_inner())
                .collect(),
            feasible,
            selected_variable: constraint
                .stage
                .selected_variable
                .into_iter()
                .map(|(id, variable_id)| {
                    (
                        id.into_inner(),
                        crate::v2::SampledSelectedVariable {
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

impl crate::v2::SampledOneHotConstraint {
    /// Restore the SDK metric from values supplied by the enclosing Solution or
    /// SampleSet. Structural wire rows contain member IDs, not their values.
    pub(crate) fn parse_with_values(
        self,
        mut value_of: impl FnMut(SampleID, VariableID) -> Option<f64>,
        atol: ATol,
    ) -> Result<SampledOneHotConstraint, ParseError> {
        let message = "ommx.v2.SampledOneHotConstraint";
        validate_atol(atol).map_err(ParseError::new)?;
        let variables =
            crate::v2_io::variable_id_set_from_v2(self.variables, message, "variables")?;
        if variables.is_empty() {
            return Err(ParseError::new(OneHotConstraintError::EmptyVariables)
                .context(message, "variables"));
        }
        let selected_variable =
            crate::v2_io::sampled_selected_variable_map_from_v2(self.selected_variable);
        if selected_variable
            .values()
            .flatten()
            .any(|id| !variables.contains(id))
        {
            return Err(ParseError::new(crate::error!(
                "One-hot selected_variable values must be members of variables"
            ))
            .context(message, "selected_variable"));
        }
        let feasible = crate::v2_io::sample_bool_map_from_v2(self.feasible);
        if sample_ids_from_map(&feasible) != sample_ids_from_map(&selected_variable) {
            return Err(ParseError::new(crate::error!(
                "feasible and selected_variable sample IDs must match"
            ))
            .context(message, "feasible"));
        }
        for id in feasible.keys() {
            if feasible[id] != selected_variable[id].is_some() {
                return Err(ParseError::new(crate::error!(
                    "One-hot feasible must be true exactly when selected_variable is set"
                ))
                .context(message, "selected_variable"));
            }
        }
        let constraint = OneHotConstraint::new(variables)
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
        Ok(OneHotConstraint {
            variables: constraint.variables,
            stage: OneHotSampledData {
                activation_atol: atol,
                violations,
                selected_variable,
                used_decision_variable_ids: crate::v2_io::variable_id_set_from_v2(
                    self.used_decision_variable_ids,
                    message,
                    "used_decision_variable_ids",
                )?,
            },
        })
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

/// Select a member to set to one in a closest OneHot assignment.
/// Shared with result diagnostics so evaluation and repair use the same tie rule.
/// Callers supply finite values; numerically equal values, including signed zeros,
/// are resolved by the largest variable ID.
fn select_one_hot_member(
    values: impl IntoIterator<Item = (VariableID, f64)>,
) -> Option<VariableID> {
    values
        .into_iter()
        .max_by(|(id_a, a), (id_b, b)| {
            if a == b {
                id_a.cmp(id_b)
            } else {
                a.total_cmp(b)
            }
        })
        .map(|(id, _)| id)
}

impl<S: Stage<Self>> OneHotConstraint<S> {
    /// Evaluate structural member values supplied by a state or a validated host.
    /// The constraint owns both its metric and the selected-variable diagnostic;
    /// Solution and SampleSet use this same operation when validating restored rows.
    pub(crate) fn evaluate_members(
        &self,
        mut value_of: impl FnMut(VariableID) -> Option<f64>,
        atol: ATol,
    ) -> crate::Result<(f64, Option<VariableID>)> {
        validate_atol(atol)?;
        let mut values = Vec::with_capacity(self.variables.len());
        for &id in &self.variables {
            let value = value_of(id).ok_or_else(|| {
                crate::error!("Variable {id:?} not found in state for one-hot constraint")
            })?;
            crate::ensure!(
                value.is_finite(),
                "Variable {id:?} in one-hot constraint must be finite (value={value})"
            );
            values.push((id, value));
        }
        let selected = select_one_hot_member(values.iter().copied()).ok_or_else(|| {
            crate::error!("one-hot constraints must contain at least one variable")
        })?;
        // Largest absolute adjustment for the repair that minimizes this maximum.
        let violation = values
            .iter()
            .map(|&(id, value)| {
                if id == selected {
                    (value - 1.0).abs()
                } else {
                    value.abs()
                }
            })
            .fold(0.0, f64::max);
        let selected_variable = if crate::constraint_type::violation_is_feasible(violation, atol) {
            Some(selected)
        } else {
            None
        };
        Ok((violation, selected_variable))
    }
}

impl EvaluatedOneHotConstraint {
    /// Largest variable adjustment needed to satisfy this constraint.
    ///
    /// Choose the repair that makes this largest absolute adjustment as small as
    /// possible. For `(0.9, 0.2, 0)`, repairing to `(1, 0, 0)` requires adjustments
    /// `(0.1, 0.2, 0)`, giving violation `0.2`. Mathematically, this is the L∞
    /// distance to a one-hot assignment; variable bounds and kinds are checked separately.
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
    fn test_create_one_hot_constraint() {
        let vars: BTreeSet<_> = [1, 2, 3].into_iter().map(VariableID::from).collect();
        let c = OneHotConstraint::new(vars.clone()).unwrap();
        assert_eq!(c.variables, vars);
    }

    #[test]
    fn one_hot_constraint_rejects_empty_variable_set() {
        let err = OneHotConstraint::new(BTreeSet::new()).unwrap_err();
        assert!(matches!(
            err.downcast_ref::<OneHotConstraintError>(),
            Some(OneHotConstraintError::EmptyVariables)
        ));
    }

    #[test]
    fn parse_v2_preserves_empty_variables_signal() {
        let parse_error = crate::v2::OneHotConstraint { variables: vec![] }
            .parse(&())
            .unwrap_err();

        assert!(matches!(
            parse_error
                .source()
                .and_then(|error| error.downcast_ref::<OneHotConstraintError>()),
            Some(OneHotConstraintError::EmptyVariables)
        ));
    }

    #[test]
    fn parse_v2_evaluated_preserves_empty_variables_signal() {
        let parse_error = crate::v2::EvaluatedOneHotConstraint::default()
            .parse_with_values(|_| None, ATol::default())
            .unwrap_err();

        assert!(matches!(
            parse_error
                .source()
                .and_then(|error| error.downcast_ref::<OneHotConstraintError>()),
            Some(OneHotConstraintError::EmptyVariables)
        ));
    }

    #[test]
    fn parse_v2_sampled_preserves_empty_variables_signal() {
        let parse_error = crate::v2::SampledOneHotConstraint::default()
            .parse_with_values(|_, _| None, ATol::default())
            .unwrap_err();

        assert!(matches!(
            parse_error
                .source()
                .and_then(|error| error.downcast_ref::<OneHotConstraintError>()),
            Some(OneHotConstraintError::EmptyVariables)
        ));
    }

    #[test]
    fn parse_v2_evaluated_invalid_selected_variable_is_an_ordinary_error() {
        let proto = crate::v2::EvaluatedOneHotConstraint {
            variables: vec![1],
            feasible: true,
            selected_variable: Some(2),
            used_decision_variable_ids: vec![],
        };

        let err = proto
            .parse_with_values(|_| None, ATol::default())
            .unwrap_err();

        assert!(!error_chain_contains::<crate::RawParseError>(&err));
        assert!(!error_chain_contains::<OneHotConstraintError>(&err));
        assert!(err
            .to_string()
            .contains("selected_variable must be a member of variables"));
    }

    #[test]
    fn test_display() {
        let vars: BTreeSet<_> = [1, 2, 3].into_iter().map(VariableID::from).collect();
        let c = OneHotConstraint::new(vars).unwrap();
        let s = format!("{}", c);
        assert!(s.contains("OneHotConstraint"));
        assert!(s.contains("x1"));
        assert!(s.contains("x2"));
        assert!(s.contains("x3"));
    }

    #[test]
    fn test_constraint_type_impl() {
        let vars: BTreeSet<_> = [1, 2].into_iter().map(VariableID::from).collect();
        let c = OneHotConstraint::new(vars).unwrap();
        let _: <OneHotConstraint as ConstraintType>::Created = c;
    }
}
