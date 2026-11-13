//! The regular constraint's type mapping and evaluated-row contracts.

use crate::{
    Constraint, ConstraintID, ConstraintType, EvaluatedConstraint, EvaluatedConstraintData,
    SampleID, SampleIDSet, SampledConstraint, SampledConstraintData, VariableIDSet,
};

impl EvaluatedConstraintData for EvaluatedConstraint {
    type ID = ConstraintID;
    fn violation(&self) -> f64 {
        self.equality.violation(self.stage.evaluated_value)
    }

    fn used_decision_variable_ids(&self) -> &VariableIDSet {
        &self.stage.used_decision_variable_ids
    }
}

impl SampledConstraintData for SampledConstraint {
    type ID = ConstraintID;
    type Evaluated = EvaluatedConstraint;
    fn violation_for(&self, sample_id: SampleID) -> Option<f64> {
        self.stage
            .evaluated_values
            .get(sample_id)
            .map(|value| self.equality.violation(*value))
    }

    fn validate_sample_ids(&self, expected: &SampleIDSet) -> std::result::Result<(), SampleIDSet> {
        if !self.stage.evaluated_values.has_same_ids(expected) {
            return Err(self.stage.evaluated_values.ids());
        }

        if let Some(dual_variables) = &self.stage.dual_variables {
            if !dual_variables.has_same_ids(expected) {
                return Err(dual_variables.ids());
            }
        }
        Ok(())
    }

    fn used_decision_variable_ids(&self) -> &VariableIDSet {
        &self.stage.used_decision_variable_ids
    }

    fn get(&self, sample_id: SampleID) -> Option<Self::Evaluated> {
        use crate::constraint::EvaluatedData;
        let evaluated_value = *self.stage.evaluated_values.get(sample_id)?;
        let dual_variable = self
            .stage
            .dual_variables
            .as_ref()
            .and_then(|duals| duals.get(sample_id))
            .copied();

        Some(crate::Constraint {
            equality: self.equality,
            stage: EvaluatedData {
                evaluated_value,
                dual_variable,
                used_decision_variable_ids: self.stage.used_decision_variable_ids.clone(),
            },
        })
    }
}

/// `Constraint` (= `Constraint<Created>`) serves as the type family for regular constraints.
impl ConstraintType for Constraint {
    const KIND: crate::ConstraintKind = crate::ConstraintKind::Regular;
    type ID = ConstraintID;
    type ViolationDiagnostics = crate::RegularViolationDiagnostics;
    type Created = Constraint;
    type Evaluated = EvaluatedConstraint;
    type Sampled = SampledConstraint;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Function;

    #[test]
    fn constraint_type_aliases() {
        let c = Constraint::equal_to_zero(Function::Zero);
        let _: <Constraint as ConstraintType>::Created = c;
    }
}
