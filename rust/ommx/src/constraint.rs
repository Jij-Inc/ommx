mod approx;
mod arbitrary;
mod context_store;
mod evaluate;
mod logical_memory;
mod parse;
mod reduce_binary_power;
pub(crate) mod stage;

pub use context_store::ConstraintContextStore;

use crate::logical_memory::LogicalMemoryProfile;
use crate::{ATol, Function, Parse, ParseError, RawParseError, SampleID, VariableID};
pub use arbitrary::*;
use derive_more::{Deref, From};
use fnv::FnvHashSet;
pub use stage::{
    Created, CreatedData, Evaluated, EvaluatedData, RemovedReason, Sampled as SampledStage,
    SampledData, Stage,
};
// The sampled lifecycle marker is re-exported as `SampledStage` to avoid a
// name collision with `crate::Sampled<T>` (the sampled-values container).

/// Constraint equality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, LogicalMemoryProfile)]
pub enum Equality {
    /// $f(x) = 0$ type constraint.
    EqualToZero,
    /// $f(x) \leq 0$ type constraint.
    LessThanOrEqualToZero,
}

/// ID for constraint
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
    LogicalMemoryProfile,
)]
#[serde(transparent)]
pub struct ConstraintID(u64);

impl std::fmt::Debug for ConstraintID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ConstraintID({})", self.0)
    }
}

impl std::fmt::Display for ConstraintID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl ConstraintID {
    pub fn into_inner(self) -> u64 {
        self.0
    }
}

impl From<ConstraintID> for u64 {
    fn from(id: ConstraintID) -> Self {
        id.0
    }
}

/// One step in a constraint's transformation history.
///
/// For example, when an indicator constraint with indicator=1 is propagated,
/// it is promoted to a regular `Constraint` with a provenance step recording
/// the original indicator constraint ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provenance {
    IndicatorConstraint(crate::IndicatorConstraintID),
    OneHotConstraint(crate::OneHotConstraintID),
    Sos1Constraint(crate::Sos1ConstraintID),
}

/// Constraint context outside the intrinsic mathematical constraint.
///
/// [`crate::ModelingLabel`] describes the original mathematical-model notation
/// for the constraint family. `provenance` is separate transformation lineage,
/// not part of that label.
#[derive(Debug, Clone, PartialEq, Default, LogicalMemoryProfile)]
pub struct ConstraintContext {
    pub label: crate::ModelingLabel,
    /// Chain of transformations that produced this constraint.
    ///
    /// Empty for constraints that were directly authored. When a constraint is
    /// transformed from another (e.g. an indicator constraint promoted to a
    /// regular constraint), a new [`Provenance`] entry is appended. Each entry
    /// records the identity of the constraint that existed just before the
    /// transformation. Older entries come first, newer last.
    pub provenance: Vec<Provenance>,
}

impl From<Provenance> for crate::v2::Provenance {
    fn from(provenance: Provenance) -> Self {
        use crate::v2::provenance::Source;

        let source = match provenance {
            Provenance::IndicatorConstraint(id) => Source::IndicatorConstraintId(id.into_inner()),
            Provenance::OneHotConstraint(id) => Source::OneHotConstraintId(id.into_inner()),
            Provenance::Sos1Constraint(id) => Source::Sos1ConstraintId(id.into_inner()),
        };
        Self {
            source: Some(source),
        }
    }
}

impl Parse for crate::v2::Provenance {
    type Output = Provenance;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        use crate::v2::provenance::Source;

        match self.source.ok_or(RawParseError::MissingField {
            message: "ommx.v2.Provenance",
            field: "source",
        })? {
            Source::IndicatorConstraintId(id) => Ok(Provenance::IndicatorConstraint(
                crate::IndicatorConstraintID::from(id),
            )),
            Source::OneHotConstraintId(id) => Ok(Provenance::OneHotConstraint(
                crate::OneHotConstraintID::from(id),
            )),
            Source::Sos1ConstraintId(id) => Ok(Provenance::Sos1Constraint(
                crate::Sos1ConstraintID::from(id),
            )),
        }
    }
}

impl From<ConstraintContext> for crate::v2::ConstraintContext {
    fn from(context: ConstraintContext) -> Self {
        Self {
            label: Some(context.label.into()),
            provenance: context.provenance.into_iter().map(Into::into).collect(),
        }
    }
}

impl Parse for crate::v2::ConstraintContext {
    type Output = ConstraintContext;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let mut provenance = Vec::with_capacity(self.provenance.len());
        for value in self.provenance {
            provenance.push(value.parse_as(&(), "ommx.v2.ConstraintContext", "provenance")?);
        }
        Ok(ConstraintContext {
            label: self.label.map(Into::into).unwrap_or_default(),
            provenance,
        })
    }
}

impl From<crate::v2::RemovedReason> for RemovedReason {
    fn from(reason: crate::v2::RemovedReason) -> Self {
        Self {
            reason: reason.reason,
            parameters: reason.parameters.into_iter().collect(),
        }
    }
}

/// A constraint parameterized by its lifecycle stage.
///
/// Holds only the constraint's intrinsic data (`equality` plus stage-specific
/// data in `stage`). Modeling labels and transformation provenance live on the
/// enclosing collection's [`ConstraintContextStore`] keyed by id; per-element
/// storage was retired in the v3 context redesign.
///
/// The constraint's [`ConstraintID`] is not stored in this struct — it is
/// held by the enclosing collection (e.g. the `BTreeMap` key in
/// [`Instance`]), which is the single source of truth. Standalone
/// constraints are identity-less until inserted into a collection.
///
/// [`Instance`]: crate::Instance
/// [`ConstraintContextStore`]: crate::ConstraintContextStore
#[derive(Debug, Clone, PartialEq)]
pub struct Constraint<S: Stage<Self> = Created> {
    pub equality: Equality,
    pub stage: S::Data,
}

// ===== Created stage (the "definition" form) =====

impl Constraint<Created> {
    /// Access the constraint function.
    pub fn function(&self) -> &Function {
        &self.stage.function
    }

    /// Mutable access to the constraint function.
    pub fn function_mut(&mut self) -> &mut Function {
        &mut self.stage.function
    }

    pub fn equal_to_zero(function: Function) -> Self {
        Self {
            equality: Equality::EqualToZero,
            stage: CreatedData { function },
        }
    }

    pub fn less_than_or_equal_to_zero(function: Function) -> Self {
        Self {
            equality: Equality::LessThanOrEqualToZero,
            stage: CreatedData { function },
        }
    }
}

impl From<Constraint<Created>> for crate::v2::RegularConstraint {
    fn from(constraint: Constraint<Created>) -> Self {
        Self {
            equality: constraint.equality.into(),
            function: Some(constraint.stage.function.into()),
        }
    }
}

impl Parse for crate::v2::RegularConstraint {
    type Output = Constraint<Created>;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v2.RegularConstraint";
        let equality = crate::v1::Equality::try_from(self.equality)
            .map_err(|_| RawParseError::UnknownEnumValue {
                enum_name: "ommx.v1.Equality",
                value: self.equality,
            })
            .map_err(|e| ParseError::from(e).context(message, "equality"))?
            .parse_as(&(), message, "equality")?;
        let function = self
            .function
            .ok_or(RawParseError::MissingField {
                message,
                field: "function",
            })?
            .parse_as(&(), message, "function")?;
        Ok(Constraint {
            equality,
            stage: CreatedData { function },
        })
    }
}

impl std::fmt::Display for Constraint<Created> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let equality_symbol = match self.equality {
            Equality::EqualToZero => "==",
            Equality::LessThanOrEqualToZero => "<=",
        };
        write!(
            f,
            "Constraint({} {} 0)",
            self.stage.function, equality_symbol
        )
    }
}

// ===== Evaluated stage =====

/// Type alias for an evaluated constraint.
pub type EvaluatedConstraint = Constraint<Evaluated>;

fn feasible_from_evaluated_value(equality: Equality, evaluated_value: f64, atol: ATol) -> bool {
    match equality {
        Equality::EqualToZero => evaluated_value.abs() < *atol,
        Equality::LessThanOrEqualToZero => evaluated_value < *atol,
    }
}

fn validate_feasible_from_evaluated_value(
    equality: Equality,
    evaluated_value: f64,
    provided_feasible: bool,
    atol: ATol,
    message: &'static str,
) -> Result<(), ParseError> {
    let computed_feasible = feasible_from_evaluated_value(equality, evaluated_value, atol);
    if provided_feasible != computed_feasible {
        return Err(RawParseError::InvalidInstance(format!(
            "Inconsistent constraint feasibility: provided={provided_feasible}, computed={computed_feasible}",
        ))
        .context(message, "feasible"));
    }
    Ok(())
}

impl From<EvaluatedConstraint> for crate::v2::EvaluatedRegularConstraint {
    fn from(constraint: EvaluatedConstraint) -> Self {
        Self {
            equality: constraint.equality.into(),
            evaluated_value: constraint.stage.evaluated_value,
            feasible: constraint.stage.feasible,
            used_decision_variable_ids: constraint
                .stage
                .used_decision_variable_ids
                .into_iter()
                .map(|id| id.into_inner())
                .collect(),
            dual_variable: constraint.stage.dual_variable,
        }
    }
}

impl Parse for crate::v2::EvaluatedRegularConstraint {
    type Output = EvaluatedConstraint;
    type Context = ATol;

    fn parse(self, atol: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v2.EvaluatedRegularConstraint";
        let equality = crate::v1::Equality::try_from(self.equality)
            .map_err(|_| RawParseError::UnknownEnumValue {
                enum_name: "ommx.v1.Equality",
                value: self.equality,
            })
            .map_err(|e| ParseError::from(e).context(message, "equality"))?
            .parse_as(&(), message, "equality")?;
        validate_feasible_from_evaluated_value(
            equality,
            self.evaluated_value,
            self.feasible,
            *atol,
            message,
        )?;
        Ok(Constraint {
            equality,
            stage: EvaluatedData {
                evaluated_value: self.evaluated_value,
                feasible: self.feasible,
                used_decision_variable_ids: crate::v2_io::variable_id_set_from_v2(
                    self.used_decision_variable_ids,
                    message,
                    "used_decision_variable_ids",
                )?,
                dual_variable: self.dual_variable,
            },
        })
    }
}

impl EvaluatedConstraint {
    /// Check if this constraint is feasible given a specific tolerance
    pub fn is_feasible_with_tolerance(&self, atol: crate::ATol) -> bool {
        match self.equality {
            Equality::EqualToZero => self.stage.evaluated_value.abs() < *atol,
            Equality::LessThanOrEqualToZero => self.stage.evaluated_value < *atol,
        }
    }

    /// Calculate the violation (constraint breach) value for this constraint
    ///
    /// Returns the amount by which this constraint is violated:
    /// - For `f(x) = 0`: returns `|f(x)|`
    /// - For `f(x) ≤ 0`: returns `max(0, f(x))`
    ///
    /// Returns 0.0 if the constraint is satisfied.
    pub fn violation(&self) -> f64 {
        match self.equality {
            Equality::EqualToZero => self.stage.evaluated_value.abs(),
            Equality::LessThanOrEqualToZero => self.stage.evaluated_value.max(0.0),
        }
    }
}

// ===== Sampled stage =====

/// Type alias for a sampled constraint.
pub type SampledConstraint = Constraint<SampledStage>;

impl From<SampledConstraint> for crate::v2::SampledRegularConstraint {
    fn from(constraint: SampledConstraint) -> Self {
        Self {
            equality: constraint.equality.into(),
            evaluated_values: Some(constraint.stage.evaluated_values.into()),
            feasible: constraint
                .stage
                .feasible
                .into_iter()
                .map(|(id, value)| (id.into_inner(), value))
                .collect(),
            used_decision_variable_ids: constraint
                .stage
                .used_decision_variable_ids
                .into_iter()
                .map(|id| id.into_inner())
                .collect(),
            dual_variables: constraint.stage.dual_variables.map(Into::into),
        }
    }
}

impl Parse for crate::v2::SampledRegularConstraint {
    type Output = SampledConstraint;
    type Context = ATol;

    fn parse(self, atol: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v2.SampledRegularConstraint";
        let equality = crate::v1::Equality::try_from(self.equality)
            .map_err(|_| RawParseError::UnknownEnumValue {
                enum_name: "ommx.v1.Equality",
                value: self.equality,
            })
            .map_err(|e| ParseError::from(e).context(message, "equality"))?
            .parse_as(&(), message, "equality")?;
        let evaluated_values = self
            .evaluated_values
            .ok_or(RawParseError::MissingField {
                message,
                field: "evaluated_values",
            })?
            .parse_as(&(), message, "evaluated_values")?;
        let feasible = crate::v2_io::sample_bool_map_from_v2(self.feasible);
        for (sample_id, evaluated_value) in evaluated_values.iter() {
            if let Some(provided_feasible) = feasible.get(sample_id).copied() {
                validate_feasible_from_evaluated_value(
                    equality,
                    *evaluated_value,
                    provided_feasible,
                    *atol,
                    message,
                )?;
            }
        }
        let dual_variables = self
            .dual_variables
            .map(|values| values.parse_as(&(), message, "dual_variables"))
            .transpose()?;
        Ok(Constraint {
            equality,
            stage: SampledData {
                evaluated_values,
                feasible,
                used_decision_variable_ids: crate::v2_io::variable_id_set_from_v2(
                    self.used_decision_variable_ids,
                    message,
                    "used_decision_variable_ids",
                )?,
                dual_variables,
            },
        })
    }
}

impl SampledConstraint {
    /// Check feasibility for a specific sample.
    ///
    /// Returns [`None`] if `sample_id` is not present in the sampled data.
    pub fn is_feasible(&self, sample_id: SampleID, atol: crate::ATol) -> Option<bool> {
        let evaluated_value = *self.stage.evaluated_values.get(sample_id)?;

        Some(match self.equality {
            Equality::EqualToZero => evaluated_value.abs() < *atol,
            Equality::LessThanOrEqualToZero => evaluated_value < *atol,
        })
    }

    /// Get all sample IDs that are feasible
    pub fn feasible_ids(&self, atol: crate::ATol) -> FnvHashSet<SampleID> {
        self.stage
            .evaluated_values
            .iter()
            .filter_map(|(sample_id, evaluated_value)| {
                let feasible = match self.equality {
                    Equality::EqualToZero => evaluated_value.abs() < *atol,
                    Equality::LessThanOrEqualToZero => *evaluated_value < *atol,
                };
                if feasible {
                    Some(*sample_id)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get all sample IDs that are infeasible
    pub fn infeasible_ids(&self, atol: crate::ATol) -> FnvHashSet<SampleID> {
        self.stage
            .evaluated_values
            .iter()
            .filter_map(|(sample_id, evaluated_value)| {
                let feasible = match self.equality {
                    Equality::EqualToZero => evaluated_value.abs() < *atol,
                    Equality::LessThanOrEqualToZero => *evaluated_value < *atol,
                };
                if !feasible {
                    Some(*sample_id)
                } else {
                    None
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Coefficient, Evaluate};

    #[test]
    fn test_violation_equality_positive() {
        let constraint =
            Constraint::equal_to_zero(Function::Constant(Coefficient::try_from(2.5).unwrap()));
        let state = crate::v1::State::default();
        let evaluated = constraint.evaluate(&state, crate::ATol::default()).unwrap();
        assert_eq!(evaluated.violation(), 2.5);
    }

    #[test]
    fn test_violation_equality_negative() {
        let constraint =
            Constraint::equal_to_zero(Function::Constant(Coefficient::try_from(-3.0).unwrap()));
        let state = crate::v1::State::default();
        let evaluated = constraint.evaluate(&state, crate::ATol::default()).unwrap();
        assert_eq!(evaluated.violation(), 3.0);
    }

    #[test]
    fn test_violation_equality_near_zero() {
        let constraint =
            Constraint::equal_to_zero(Function::Constant(Coefficient::try_from(0.0001).unwrap()));
        let state = crate::v1::State::default();
        let evaluated = constraint.evaluate(&state, crate::ATol::default()).unwrap();
        assert_eq!(evaluated.violation(), 0.0001);
    }

    #[test]
    fn test_violation_inequality_violated() {
        let constraint = Constraint::less_than_or_equal_to_zero(Function::Constant(
            Coefficient::try_from(1.5).unwrap(),
        ));
        let state = crate::v1::State::default();
        let evaluated = constraint.evaluate(&state, crate::ATol::default()).unwrap();
        assert_eq!(evaluated.violation(), 1.5);
    }

    #[test]
    fn test_violation_inequality_satisfied() {
        let constraint = Constraint::less_than_or_equal_to_zero(Function::Constant(
            Coefficient::try_from(-1.0).unwrap(),
        ));
        let state = crate::v1::State::default();
        let evaluated = constraint.evaluate(&state, crate::ATol::default()).unwrap();
        assert_eq!(evaluated.violation(), 0.0);
    }

    #[test]
    fn test_violation_inequality_near_boundary() {
        let constraint = Constraint::less_than_or_equal_to_zero(Function::Constant(
            Coefficient::try_from(0.0001).unwrap(),
        ));
        let state = crate::v1::State::default();
        let evaluated = constraint.evaluate(&state, crate::ATol::default()).unwrap();
        assert_eq!(evaluated.violation(), 0.0001);
    }
}
