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

    #[error("Variable {id:?} cannot be both fixed (substituted_value set) and dependent")]
    FixedAndDependentVariable { id: VariableID },

    #[error(
        "Fixed variable {id:?} (substituted_value set) cannot be used in objectives or constraints"
    )]
    FixedVariableUsed { id: VariableID },

    #[error("Required field is missing: {field}")]
    MissingRequiredField { field: &'static str },

    #[error(
        "Constraint ID {id:?} is in both constraints and removed_constraints, but they must be disjoint"
    )]
    OverlappingConstraintID { id: ConstraintID },

    #[error("Variable ID {id:?} in decision_variable_dependency is not in decision_variables")]
    UndefinedDependentVariableID { id: VariableID },

    #[error("Decision variable map key {key:?} does not match value's id {value_id:?}")]
    InconsistentDecisionVariableID {
        key: VariableID,
        value_id: VariableID,
    },

    #[error("Constraint map key {key:?} does not match value's id {value_id:?}")]
    InconsistentConstraintID {
        key: ConstraintID,
        value_id: ConstraintID,
    },

    #[error("Removed constraint map key {key:?} does not match value's id {value_id:?}")]
    InconsistentRemovedConstraintID {
        key: ConstraintID,
        value_id: ConstraintID,
    },

    #[error("Parameter map key {key:?} does not match value's id {value_id}")]
    InconsistentParameterID { key: VariableID, value_id: u64 },
}
