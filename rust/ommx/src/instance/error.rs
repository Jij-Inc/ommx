use super::*;

/// Violation of the [`Instance`] invariants.
#[derive(Debug, thiserror::Error)]
pub enum InstanceError {
    #[error("Undefined decision variable ID: {id}")]
    UndefinedDecisionVariableID { id: VariableID },
    #[error("Duplicated constraint ID: {id}")]
    UndefinedConstraintID { id: ConstraintID },
    #[error("Duplicated constraint ID: {id}")]
    DuplicatedConstraintID { id: ConstraintID },
}
