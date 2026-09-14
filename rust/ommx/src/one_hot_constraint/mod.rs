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

/// Validation failures for one-hot constraints.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum OneHotConstraintError {
    /// A one-hot constraint has no variables to select from.
    #[error("One-hot constraints must contain at least one variable")]
    EmptyVariables,
}

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
/// the metric and active-variable diagnostic against its decision-variable values.
#[derive(Debug, Clone, PartialEq)]
pub struct OneHotEvaluatedData {
    /// Tolerance used to determine the retained activation diagnostic.
    /// Feasibility queries supply their own tolerance and do not change it.
    pub activation_atol: ATol,
    pub violation: f64,
    /// Selected variable at `activation_atol`; None if infeasible at that tolerance.
    pub active_variable: Option<VariableID>,
    pub used_decision_variable_ids: VariableIDSet,
}

/// Data carried by a one-hot constraint in the Sampled stage.
///
/// Each violation is nonnegative and not NaN. The violation and active-variable
/// maps must have the same sample IDs. Evaluation constructs this data and
/// SampleSet validates the metrics and diagnostics against its sampled variables.
#[derive(Debug, Clone)]
pub struct OneHotSampledData {
    /// Tolerance used to determine the retained activation diagnostic.
    /// Feasibility queries supply their own tolerance and do not change it.
    pub activation_atol: ATol,
    pub violations: crate::Sampled<f64>,
    /// Selected variable in a nearest feasible one-hot vector for each sample.
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

        Some(OneHotConstraint {
            variables: self.variables.clone(),
            stage: OneHotEvaluatedData {
                activation_atol: self.stage.activation_atol,
                violation,
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

impl crate::v2::EvaluatedOneHotConstraint {
    /// Restore the SDK metric from values supplied by the enclosing Solution or
    /// SampleSet. Structural wire rows contain member IDs, not their values.
    pub(crate) fn parse_with_values(
        self,
        value_of: impl FnMut(VariableID) -> Option<f64>,
        atol: ATol,
    ) -> Result<EvaluatedOneHotConstraint, ParseError> {
        let message = "ommx.v2.EvaluatedOneHotConstraint";
        let variables =
            crate::v2_io::variable_id_set_from_v2(self.variables, message, "variables")?;
        if variables.is_empty() {
            return Err(ParseError::new(OneHotConstraintError::EmptyVariables)
                .context(message, "variables"));
        }
        let active_variable = self.active_variable.map(VariableID::from);
        if active_variable.is_some_and(|id| !variables.contains(&id)) {
            return Err(ParseError::new(crate::error!(
                "One-hot active_variable must be a member of variables"
            ))
            .context(message, "active_variable"));
        }
        if self.feasible != active_variable.is_some() {
            return Err(ParseError::new(crate::error!(
                "One-hot feasible must be true exactly when active_variable is set"
            ))
            .context(message, "active_variable"));
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

impl crate::v2::SampledOneHotConstraint {
    /// Restore the SDK metric from values supplied by the enclosing Solution or
    /// SampleSet. Structural wire rows contain member IDs, not their values.
    pub(crate) fn parse_with_values(
        self,
        mut value_of: impl FnMut(SampleID, VariableID) -> Option<f64>,
        atol: ATol,
    ) -> Result<SampledOneHotConstraint, ParseError> {
        let message = "ommx.v2.SampledOneHotConstraint";
        let variables =
            crate::v2_io::variable_id_set_from_v2(self.variables, message, "variables")?;
        if variables.is_empty() {
            return Err(ParseError::new(OneHotConstraintError::EmptyVariables)
                .context(message, "variables"));
        }
        let active_variable =
            crate::v2_io::sampled_active_variable_map_from_v2(self.active_variable);
        if active_variable
            .values()
            .flatten()
            .any(|id| !variables.contains(id))
        {
            return Err(ParseError::new(crate::error!(
                "One-hot active_variable values must be members of variables"
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
            if feasible[id] != active_variable[id].is_some() {
                return Err(ParseError::new(crate::error!(
                    "One-hot feasible must be true exactly when active_variable is set"
                ))
                .context(message, "active_variable"));
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

impl<S: Stage<Self>> OneHotConstraint<S> {
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
                crate::error!("Variable {id:?} not found in state for one-hot constraint")
            })?;
            crate::ensure!(
                value.is_finite(),
                "Variable {id:?} in one-hot constraint must be finite (value={value})"
            );
            values.push((id, value));
        }
        let &(selected, _selected_value) = values
            .iter()
            .max_by(|(_, a), (_, b)| a.total_cmp(b))
            .ok_or_else(|| {
                crate::error!("one-hot constraints must contain at least one variable")
            })?;
        // Sum nonnegative terms directly: sum-minus-maximum can erase small violations.
        let violation = values
            .iter()
            .map(|&(id, value)| {
                if id == selected {
                    (value - 1.0).abs()
                } else {
                    value.abs()
                }
            })
            .fold(0.0, |total, value| total + value);
        let active_variable = if crate::constraint_type::violation_is_feasible(violation, atol) {
            Some(selected)
        } else {
            None
        };
        Ok((violation, active_variable))
    }
}

impl EvaluatedOneHotConstraint {
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
    fn parse_v2_evaluated_invalid_active_variable_is_an_ordinary_error() {
        let proto = crate::v2::EvaluatedOneHotConstraint {
            variables: vec![1],
            feasible: true,
            active_variable: Some(2),
            used_decision_variable_ids: vec![],
        };

        let err = proto
            .parse_with_values(|_| None, ATol::default())
            .unwrap_err();

        assert!(!error_chain_contains::<crate::RawParseError>(&err));
        assert!(!error_chain_contains::<OneHotConstraintError>(&err));
        assert!(err
            .to_string()
            .contains("active_variable must be a member of variables"));
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
