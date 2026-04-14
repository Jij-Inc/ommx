mod evaluate;

use crate::{
    constraint::{
        stage, ConstraintMetadata, Created, CreatedData, Equality, Evaluated, EvaluatedData,
        Removed, RemovedData, SampledData, Stage,
    },
    constraint_type::{ConstraintType, EvaluatedConstraintBehavior, SampledConstraintBehavior},
    Function, SampleID, VariableID,
};
use derive_more::{Deref, From};

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

// ===== Stage implementations =====
// Reuse the same stage data types as regular Constraint.

impl Stage<IndicatorConstraint<Created>> for Created {
    type Data = CreatedData;
}

impl Stage<IndicatorConstraint<Removed>> for Removed {
    type Data = RemovedData;
}

impl Stage<IndicatorConstraint<Evaluated>> for Evaluated {
    type Data = EvaluatedData;
}

impl Stage<IndicatorConstraint<stage::Sampled>> for stage::Sampled {
    type Data = SampledData;
}

// ===== Type aliases =====

pub type RemovedIndicatorConstraint = IndicatorConstraint<Removed>;
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
    fn is_removed(&self) -> bool {
        self.stage.removed_reason.is_some()
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
    fn is_removed(&self) -> bool {
        self.stage.removed_reason.is_some()
    }
    fn get(
        &self,
        sample_id: SampleID,
    ) -> Result<Self::Evaluated, crate::sampled::UnknownSampleIDError> {
        let evaluated_value = *self.stage.evaluated_values.get(sample_id)?;
        let dual_variable = self
            .stage
            .dual_variables
            .as_ref()
            .and_then(|duals| duals.get(sample_id).ok())
            .copied();
        let feasible = *self.stage.feasible.get(&sample_id).unwrap_or(&false);

        Ok(IndicatorConstraint {
            id: self.id,
            indicator_variable: self.indicator_variable,
            equality: self.equality,
            metadata: self.metadata.clone(),
            stage: EvaluatedData {
                evaluated_value,
                dual_variable,
                feasible,
                used_decision_variable_ids: self.stage.used_decision_variable_ids.clone(),
                removed_reason: self.stage.removed_reason.clone(),
            },
        })
    }
}

// ===== ConstraintType =====

impl ConstraintType for IndicatorConstraint {
    type ID = IndicatorConstraintID;
    type Created = IndicatorConstraint;
    type Removed = RemovedIndicatorConstraint;
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

// ===== Removed stage =====

impl RemovedIndicatorConstraint {
    /// Access the constraint function.
    pub fn function(&self) -> &Function {
        &self.stage.function
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
