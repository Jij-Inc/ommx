mod evaluate;

use crate::{
    constraint::{stage, ConstraintMetadata, Created, CreatedData, Equality, Evaluated, Stage},
    constraint_type::{ConstraintType, EvaluatedConstraintBehavior, SampledConstraintBehavior},
    Function, SampleID, VariableID, VariableIDSet,
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

/// An indicator constraint: `indicator_variable = 1 → f(x) <= 0` (or `= 0`).
///
/// When the binary indicator variable is 0, the constraint is unconditionally satisfied.
/// When it is 1, the constraint `f(x) <= 0` (or `f(x) = 0`) must hold.
#[derive(Debug, Clone, PartialEq)]
pub struct IndicatorConstraint<S: Stage<Self> = Created> {
    pub id: IndicatorConstraintID,
    /// The binary decision variable that activates this constraint.
    pub indicator_variable: VariableID,
    pub equality: Equality,
    pub metadata: ConstraintMetadata,
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
    fn constraint_id(&self) -> IndicatorConstraintID {
        self.id
    }
    fn is_feasible(&self) -> bool {
        self.stage.feasible
    }
}

impl SampledConstraintBehavior for SampledIndicatorConstraint {
    type ID = IndicatorConstraintID;
    type Evaluated = EvaluatedIndicatorConstraint;

    fn constraint_id(&self) -> IndicatorConstraintID {
        self.id
    }
    fn is_feasible_for(&self, sample_id: SampleID) -> Option<bool> {
        self.stage.feasible.get(&sample_id).copied()
    }
    fn get(
        &self,
        sample_id: SampleID,
    ) -> Result<Self::Evaluated, crate::sampled::UnknownSampleIDError> {
        let evaluated_value = *self.stage.evaluated_values.get(sample_id)?;
        let feasible = *self.stage.feasible.get(&sample_id).unwrap_or(&false);
        let indicator_active = *self
            .stage
            .indicator_active
            .get(&sample_id)
            .unwrap_or(&false);

        Ok(IndicatorConstraint {
            id: self.id,
            indicator_variable: self.indicator_variable,
            equality: self.equality,
            metadata: self.metadata.clone(),
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
    pub fn new(
        id: IndicatorConstraintID,
        indicator_variable: VariableID,
        equality: Equality,
        function: Function,
    ) -> Self {
        Self {
            id,
            indicator_variable,
            equality,
            metadata: ConstraintMetadata::default(),
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
            IndicatorConstraintID::from(1),
            VariableID::from(10),
            Equality::LessThanOrEqualToZero,
            Function::from(linear!(1) + coeff!(-5.0)),
        );
        assert_eq!(ic.id, IndicatorConstraintID::from(1));
        assert_eq!(ic.indicator_variable, VariableID::from(10));
        assert_eq!(ic.equality, Equality::LessThanOrEqualToZero);
    }

    #[test]
    fn test_display() {
        let ic = IndicatorConstraint::new(
            IndicatorConstraintID::from(1),
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
        let ic = IndicatorConstraint::new(
            IndicatorConstraintID::from(1),
            VariableID::from(10),
            Equality::EqualToZero,
            Function::Zero,
        );
        let _: <IndicatorConstraint as ConstraintType>::Created = ic;
    }
}
