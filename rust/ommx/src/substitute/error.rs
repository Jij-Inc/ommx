use crate::decision_variable::VariableID;

/// Error types that can occur during substitution operations.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SubstitutionError {
    /// Error indicating that a recursive assignment was attempted.
    #[error("Recursive assignment detected: variable {var_id} cannot be assigned to a function that depends on itself")]
    RecursiveAssignment { var_id: VariableID },

    /// Error indicating that a cycle was detected in the assignment graph.
    #[error("Cyclic assignment detected: circular dependency found in variable assignments")]
    CyclicAssignmentDetected,

    /// `ParametricInstance::substitute` only substitutes decision variables.
    #[error(
        "Cannot substitute parameter {parameter:?}; use with_parameters to assign parameter values"
    )]
    ParameterSubstitution { parameter: VariableID },

    /// Substitution assignments may only reference IDs defined by the host.
    #[error("Undefined variable ID is used in substitution: {variable:?}")]
    UndefinedSubstitutionVariable { variable: VariableID },
}
