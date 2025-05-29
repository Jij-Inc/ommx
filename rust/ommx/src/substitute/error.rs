use crate::decision_variable::VariableID;

/// Error types that can occur during substitution operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SubstitutionError {
    /// Error indicating that a recursive assignment was attempted.
    #[error("Recursive assignment detected: variable {var_id} cannot be assigned to a function that depends on itself")]
    RecursiveAssignment { var_id: VariableID },
    
    /// Error indicating that a cycle was detected in the assignment graph.
    #[error("Cyclic assignment detected: variable {var_id} participates in a circular dependency")]
    CyclicAssignmentDetected { var_id: VariableID },
}

/// Legacy alias for backward compatibility
pub type RecursiveAssignmentError = SubstitutionError;
