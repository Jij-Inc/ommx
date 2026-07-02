mod evaluate;

use crate::{
    constraint::{stage, Created, CreatedData, Equality, Evaluated, Stage},
    constraint_type::{
        sample_ids_from_map, ConstraintType, EvaluatedConstraintBehavior, SampledConstraintBehavior,
    },
    Function, Parse, ParseError, RawParseError, SampleID, SampleIDSet, VariableID, VariableIDSet,
};
use derive_more::{Deref, From};
use std::collections::BTreeMap;

/// ID for indicator constraints, independent from regular [`ConstraintID`](crate::ConstraintID).
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
pub struct IndicatorConstraintID(u64);

impl std::fmt::Debug for IndicatorConstraintID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "IndicatorConstraintID({})", self.0)
    }
}

impl std::fmt::Display for IndicatorConstraintID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl IndicatorConstraintID {
    pub fn into_inner(self) -> u64 {
        self.0
    }
}

impl From<IndicatorConstraintID> for u64 {
    fn from(id: IndicatorConstraintID) -> Self {
        id.0
    }
}

/// An indicator constraint: `indicator_variable = 1 → f(x) <= 0` (or `= 0`).
///
/// When the binary indicator variable is 0, the constraint is unconditionally satisfied.
/// When it is 1, the constraint `f(x) <= 0` (or `f(x) = 0`) must hold.
///
/// The constraint's [`IndicatorConstraintID`] is not stored in this struct — it is held
/// by the enclosing collection (e.g. the `BTreeMap` key in [`Instance`]).
/// Modeling labels and provenance live on the enclosing collection's
/// [`ConstraintContextStore`](crate::ConstraintContextStore); per-element
/// context storage was retired in the v3 redesign.
///
/// [`Instance`]: crate::Instance
#[derive(Debug, Clone, PartialEq)]
pub struct IndicatorConstraint<S: Stage<Self> = Created> {
    /// The binary decision variable that activates this constraint.
    pub indicator_variable: VariableID,
    pub equality: Equality,
    pub stage: S::Data,
}

// ===== Indicator-specific stage data =====

/// Data carried by an indicator constraint in the Evaluated stage.
///
/// Unlike regular [`EvaluatedData`](crate::constraint::EvaluatedData), this records
/// whether the indicator variable was active and does not include a dual variable
/// (duals are not well-defined for indicator constraints).
#[derive(Debug, Clone, PartialEq)]
pub struct IndicatorEvaluatedData {
    pub evaluated_value: f64,
    pub feasible: bool,
    /// Whether the indicator variable was active (ON) at evaluation time.
    pub indicator_active: bool,
    pub used_decision_variable_ids: VariableIDSet,
}

/// Data carried by an indicator constraint in the Sampled stage.
#[derive(Debug, Clone)]
pub struct IndicatorSampledData {
    pub evaluated_values: crate::Sampled<f64>,
    pub feasible: BTreeMap<SampleID, bool>,
    /// Whether the indicator variable was active (ON) for each sample.
    pub indicator_active: BTreeMap<SampleID, bool>,
    pub used_decision_variable_ids: VariableIDSet,
}

// ===== Stage implementations =====

impl Stage<IndicatorConstraint<Created>> for Created {
    type Data = CreatedData;
}

impl Stage<IndicatorConstraint<Evaluated>> for Evaluated {
    type Data = IndicatorEvaluatedData;
}

impl Stage<IndicatorConstraint<stage::Sampled>> for stage::Sampled {
    type Data = IndicatorSampledData;
}

// ===== Type aliases =====

pub type EvaluatedIndicatorConstraint = IndicatorConstraint<Evaluated>;
pub type SampledIndicatorConstraint = IndicatorConstraint<stage::Sampled>;

// ===== HasConstraintID =====

impl EvaluatedConstraintBehavior for EvaluatedIndicatorConstraint {
    type ID = IndicatorConstraintID;
    fn is_feasible(&self) -> bool {
        self.stage.feasible
    }

    fn used_decision_variable_ids(&self) -> &VariableIDSet {
        &self.stage.used_decision_variable_ids
    }
}

impl SampledConstraintBehavior for SampledIndicatorConstraint {
    type ID = IndicatorConstraintID;
    type Evaluated = EvaluatedIndicatorConstraint;

    fn is_feasible_for(&self, sample_id: SampleID) -> Option<bool> {
        self.stage.feasible.get(&sample_id).copied()
    }

    fn validate_sample_ids(&self, expected: &SampleIDSet) -> std::result::Result<(), SampleIDSet> {
        if !self.stage.evaluated_values.has_same_ids(expected) {
            return Err(self.stage.evaluated_values.ids());
        }
        let feasible_ids = sample_ids_from_map(&self.stage.feasible);
        if &feasible_ids != expected {
            return Err(feasible_ids);
        }
        let indicator_active_ids = sample_ids_from_map(&self.stage.indicator_active);
        if &indicator_active_ids != expected {
            return Err(indicator_active_ids);
        }
        Ok(())
    }

    fn used_decision_variable_ids(&self) -> &VariableIDSet {
        &self.stage.used_decision_variable_ids
    }

    fn get(&self, sample_id: SampleID) -> Option<Self::Evaluated> {
        let evaluated_value = *self.stage.evaluated_values.get(sample_id)?;
        let feasible = *self.stage.feasible.get(&sample_id)?;
        let indicator_active = *self.stage.indicator_active.get(&sample_id)?;

        Some(IndicatorConstraint {
            indicator_variable: self.indicator_variable,
            equality: self.equality,
            stage: IndicatorEvaluatedData {
                evaluated_value,
                feasible,
                indicator_active,
                used_decision_variable_ids: self.stage.used_decision_variable_ids.clone(),
            },
        })
    }
}

// ===== ConstraintType =====

impl ConstraintType for IndicatorConstraint {
    type ID = IndicatorConstraintID;
    type Created = IndicatorConstraint;
    type Evaluated = EvaluatedIndicatorConstraint;
    type Sampled = SampledIndicatorConstraint;
}

// ===== Created stage =====

impl IndicatorConstraint<Created> {
    /// Create a new indicator constraint.
    pub fn new(indicator_variable: VariableID, equality: Equality, function: Function) -> Self {
        Self {
            indicator_variable,
            equality,
            stage: CreatedData { function },
        }
    }

    /// Access the constraint function.
    pub fn function(&self) -> &Function {
        &self.stage.function
    }

    /// Mutable access to the constraint function.
    pub fn function_mut(&mut self) -> &mut Function {
        &mut self.stage.function
    }
}

impl From<IndicatorConstraint<Created>> for crate::v2::IndicatorConstraint {
    fn from(constraint: IndicatorConstraint<Created>) -> Self {
        Self {
            indicator_variable: constraint.indicator_variable.into_inner(),
            equality: constraint.equality.into(),
            function: Some(constraint.stage.function.into()),
        }
    }
}

impl Parse for crate::v2::IndicatorConstraint {
    type Output = IndicatorConstraint<Created>;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v2.IndicatorConstraint";
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
        Ok(IndicatorConstraint {
            indicator_variable: VariableID::from(self.indicator_variable),
            equality,
            stage: CreatedData { function },
        })
    }
}

impl From<EvaluatedIndicatorConstraint> for crate::v2::EvaluatedIndicatorConstraint {
    fn from(constraint: EvaluatedIndicatorConstraint) -> Self {
        Self {
            indicator_variable: constraint.indicator_variable.into_inner(),
            equality: constraint.equality.into(),
            evaluated_value: constraint.stage.evaluated_value,
            feasible: constraint.stage.feasible,
            indicator_active: constraint.stage.indicator_active,
            used_decision_variable_ids: constraint
                .stage
                .used_decision_variable_ids
                .into_iter()
                .map(|id| id.into_inner())
                .collect(),
        }
    }
}

impl Parse for crate::v2::EvaluatedIndicatorConstraint {
    type Output = EvaluatedIndicatorConstraint;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v2.EvaluatedIndicatorConstraint";
        let equality = crate::v1::Equality::try_from(self.equality)
            .map_err(|_| RawParseError::UnknownEnumValue {
                enum_name: "ommx.v1.Equality",
                value: self.equality,
            })
            .map_err(|e| ParseError::from(e).context(message, "equality"))?
            .parse_as(&(), message, "equality")?;
        Ok(IndicatorConstraint {
            indicator_variable: VariableID::from(self.indicator_variable),
            equality,
            stage: IndicatorEvaluatedData {
                evaluated_value: self.evaluated_value,
                feasible: self.feasible,
                indicator_active: self.indicator_active,
                used_decision_variable_ids: crate::v2_io::variable_id_set_from_v2(
                    self.used_decision_variable_ids,
                    message,
                    "used_decision_variable_ids",
                )?,
            },
        })
    }
}

impl From<SampledIndicatorConstraint> for crate::v2::SampledIndicatorConstraint {
    fn from(constraint: SampledIndicatorConstraint) -> Self {
        Self {
            indicator_variable: constraint.indicator_variable.into_inner(),
            equality: constraint.equality.into(),
            evaluated_values: Some(constraint.stage.evaluated_values.into()),
            feasible: constraint
                .stage
                .feasible
                .into_iter()
                .map(|(id, value)| (id.into_inner(), value))
                .collect(),
            indicator_active: constraint
                .stage
                .indicator_active
                .into_iter()
                .map(|(id, value)| (id.into_inner(), value))
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

impl Parse for crate::v2::SampledIndicatorConstraint {
    type Output = SampledIndicatorConstraint;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v2.SampledIndicatorConstraint";
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
        Ok(IndicatorConstraint {
            indicator_variable: VariableID::from(self.indicator_variable),
            equality,
            stage: IndicatorSampledData {
                evaluated_values,
                feasible: crate::v2_io::sample_bool_map_from_v2(self.feasible),
                indicator_active: crate::v2_io::sample_bool_map_from_v2(self.indicator_active),
                used_decision_variable_ids: crate::v2_io::variable_id_set_from_v2(
                    self.used_decision_variable_ids,
                    message,
                    "used_decision_variable_ids",
                )?,
            },
        })
    }
}

impl std::fmt::Display for IndicatorConstraint<Created> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let equality_symbol = match self.equality {
            Equality::EqualToZero => "==",
            Equality::LessThanOrEqualToZero => "<=",
        };
        write!(
            f,
            "IndicatorConstraint(x{} = 1 → {} {} 0)",
            self.indicator_variable.into_inner(),
            self.stage.function,
            equality_symbol
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, linear};

    #[test]
    fn test_create_indicator_constraint() {
        let ic = IndicatorConstraint::new(
            VariableID::from(10),
            Equality::LessThanOrEqualToZero,
            Function::from(linear!(1) + coeff!(-5.0)),
        );
        assert_eq!(ic.indicator_variable, VariableID::from(10));
        assert_eq!(ic.equality, Equality::LessThanOrEqualToZero);
    }

    #[test]
    fn test_display() {
        let ic = IndicatorConstraint::new(
            VariableID::from(10),
            Equality::LessThanOrEqualToZero,
            Function::from(linear!(1) + coeff!(-5.0)),
        );
        let s = format!("{}", ic);
        assert!(s.contains("IndicatorConstraint"));
        assert!(s.contains("x10"));
    }

    #[test]
    fn test_constraint_type_impl() {
        // Verify ConstraintType associated types compile correctly
        let ic =
            IndicatorConstraint::new(VariableID::from(10), Equality::EqualToZero, Function::Zero);
        let _: <IndicatorConstraint as ConstraintType>::Created = ic;
    }
}
