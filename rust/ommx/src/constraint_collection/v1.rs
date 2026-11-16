use super::*;
use crate::{
    v1, ATol, Constraint, ConstraintContext, ConstraintID, EvaluatedConstraint, SampledConstraint,
    SampledConstraintBehavior,
};

impl TryFrom<ConstraintCollection<Constraint>>
    for (Vec<v1::Constraint>, Vec<v1::RemovedConstraint>)
{
    type Error = crate::Error;

    fn try_from(value: ConstraintCollection<Constraint>) -> crate::Result<Self> {
        crate::v1_io::validate_inactive_metadata(
            value.inactive().values().map(|(_, metadata)| metadata),
        )?;
        let ConstraintCollection {
            active,
            inactive,
            mut context,
        } = value;
        let active = active
            .into_iter()
            .map(|(id, constraint)| constraint_to_v1(id, constraint, context.remove(id)))
            .collect();
        let inactive = inactive
            .into_iter()
            .map(|(id, (constraint, reason))| {
                inactive_constraint_to_v1(id, constraint, context.remove(id), reason)
            })
            .collect();
        Ok((active, inactive))
    }
}

fn constraint_to_v1(
    id: ConstraintID,
    value: Constraint,
    context: ConstraintContext,
) -> v1::Constraint {
    let label = context.label;
    v1::Constraint {
        id: id.into_inner(),
        equality: value.equality.into(),
        function: Some(value.stage.function.into()),
        name: label.name,
        subscripts: label.subscripts,
        parameters: label.parameters.into_iter().collect(),
        description: label.description,
    }
}

fn inactive_constraint_to_v1(
    id: ConstraintID,
    constraint: Constraint,
    context: ConstraintContext,
    inactive_metadata: InactiveConstraintMetadata,
) -> v1::RemovedConstraint {
    v1::RemovedConstraint {
        constraint: Some(constraint_to_v1(id, constraint, context)),
        removed_reason: inactive_metadata.reason,
        removed_reason_parameters: inactive_metadata.parameters.into_iter().collect(),
    }
}

impl TryFrom<EvaluatedCollection<Constraint>> for Vec<v1::EvaluatedConstraint> {
    type Error = crate::Error;

    fn try_from(value: EvaluatedCollection<Constraint>) -> crate::Result<Self> {
        crate::v1_io::validate_inactive_metadata(value.inactive_metadata().values())?;
        let EvaluatedCollection {
            constraints,
            mut inactive_metadata,
            mut context,
        } = value;
        Ok(constraints
            .into_iter()
            .map(|(id, constraint)| {
                let context = context.remove(id);
                match inactive_metadata.remove(&id) {
                    Some(reason) => evaluated_constraint_to_v1(id, constraint, context, reason),
                    None => evaluated_constraint_to_v1_active(id, constraint, context),
                }
            })
            .collect())
    }
}

fn evaluated_constraint_to_v1_active(
    id: ConstraintID,
    constraint: EvaluatedConstraint,
    context: ConstraintContext,
) -> v1::EvaluatedConstraint {
    let label = context.label;
    v1::EvaluatedConstraint {
        id: id.into_inner(),
        equality: constraint.equality.into(),
        evaluated_value: constraint.stage.evaluated_value,
        used_decision_variable_ids: constraint
            .stage
            .used_decision_variable_ids
            .into_iter()
            .map(|id| id.into_inner())
            .collect(),
        subscripts: label.subscripts,
        parameters: label.parameters.into_iter().collect(),
        name: label.name,
        description: label.description,
        dual_variable: constraint.stage.dual_variable,
        removed_reason: None,
        removed_reason_parameters: Default::default(),
    }
}

fn evaluated_constraint_to_v1(
    id: ConstraintID,
    constraint: EvaluatedConstraint,
    context: ConstraintContext,
    inactive_metadata: InactiveConstraintMetadata,
) -> v1::EvaluatedConstraint {
    let mut value = evaluated_constraint_to_v1_active(id, constraint, context);
    value.removed_reason = Some(inactive_metadata.reason);
    value.removed_reason_parameters = inactive_metadata.parameters.into_iter().collect();
    value
}

impl SampledCollection<Constraint> {
    /// SampleSet supplies the v1 parser's default tolerance so row feasibility
    /// maps use the same conditions as the serialized root maps.
    pub(crate) fn into_v1(self, atol: ATol) -> crate::Result<Vec<v1::SampledConstraint>> {
        crate::v1_io::validate_inactive_metadata(self.inactive_metadata().values())?;
        let value = self;
        let SampledCollection {
            constraints,
            mut inactive_metadata,
            mut context,
        } = value;
        Ok(constraints
            .into_iter()
            .map(|(id, constraint)| {
                let context = context.remove(id);
                match inactive_metadata.remove(&id) {
                    Some(reason) => sampled_constraint_to_v1(id, constraint, context, reason, atol),
                    None => sampled_constraint_to_v1_active(id, constraint, context, atol),
                }
            })
            .collect())
    }
}

fn sampled_constraint_to_v1_active(
    id: ConstraintID,
    constraint: SampledConstraint,
    context: ConstraintContext,
    atol: ATol,
) -> v1::SampledConstraint {
    let label = context.label;
    let feasible = constraint
        .stage
        .evaluated_values
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
    let evaluated_values: v1::SampledValues = constraint.stage.evaluated_values.into();

    v1::SampledConstraint {
        id: id.into_inner(),
        equality: constraint.equality.into(),
        name: label.name,
        subscripts: label.subscripts,
        parameters: label.parameters.into_iter().collect(),
        description: label.description,
        removed_reason: None,
        removed_reason_parameters: Default::default(),
        evaluated_values: Some(evaluated_values),
        used_decision_variable_ids: constraint
            .stage
            .used_decision_variable_ids
            .into_iter()
            .map(|id| id.into_inner())
            .collect(),
        feasible,
    }
}

fn sampled_constraint_to_v1(
    id: ConstraintID,
    constraint: SampledConstraint,
    context: ConstraintContext,
    inactive_metadata: InactiveConstraintMetadata,
    atol: ATol,
) -> v1::SampledConstraint {
    let mut value = sampled_constraint_to_v1_active(id, constraint, context, atol);
    value.removed_reason = Some(inactive_metadata.reason);
    value.removed_reason_parameters = inactive_metadata.parameters.into_iter().collect();
    value
}
