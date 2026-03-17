use super::*;

/// Violation of the [`Instance`] invariants.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum InstanceError {
    #[error("Duplicated variable ID is found in definition: {id:?}")]
    DuplicatedVariableID { id: VariableID },

    #[error("Duplicated constraint ID is found in definition: {id:?}")]
    DuplicatedConstraintID { id: ConstraintID },

    #[error("Undefined variable ID is used: {id:?}")]
    UndefinedVariableID { id: VariableID },

    #[error("Undefined constraint ID is used: {id:?}")]
    UndefinedConstraintID { id: ConstraintID },

    #[error("Non-unique variable ID is found where uniqueness is required: {id:?}")]
    NonUniqueVariableID { id: VariableID },

    #[error("Non-unique constraint ID is found where uniqueness is required: {id:?}")]
    NonUniqueConstraintID { id: ConstraintID },

    #[error("Dependent variable cannot be used in objectives or constraints: {id:?}")]
    DependentVariableUsed { id: VariableID },

    #[error("Required field is missing: {field}")]
    MissingRequiredField { field: &'static str },

    #[error(
        "Constraint ID {id:?} is in both constraints and removed_constraints, but they must be disjoint"
    )]
    OverlappingConstraintID { id: ConstraintID },

    #[error(
        "Variable ID {id:?} is in both decision_variables and decision_variable_dependency, but dependent variables must not be in decision_variables"
    )]
    OverlappingDependentVariableID { id: VariableID },
}
