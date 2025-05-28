use crate::decision_variable::VariableID;

/// Error indicating that a recursive assignment was attempted.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("Recursive assignment detected: variable {var_id} cannot be assigned to a function that depends on itself")]
pub struct RecursiveAssignmentError {
    pub var_id: VariableID,
}
