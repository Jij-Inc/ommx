//! Error types for SCIP adapter

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScipAdapterError {
    #[error("SCIP initialization failed")]
    InitializationFailed,

    #[error("SCIP solve failed with return code: {0}")]
    SolveFailed(i32),

    #[error("Unsupported variable kind: {0:?}")]
    UnsupportedVariableKind(ommx::Kind),

    #[error("Unsupported constraint equality: {0:?}")]
    UnsupportedConstraintEquality(ommx::Equality),

    #[error("Unsupported function degree: {0} (only linear and quadratic supported)")]
    UnsupportedFunctionDegree(u64),

    #[error("Variable ID {0} not found in adapter")]
    VariableNotFound(u64),

    #[error("No solution available")]
    NoSolutionAvailable,

    #[error("Problem is infeasible")]
    Infeasible,

    #[error("Problem is unbounded")]
    Unbounded,

    #[error("SCIP internal error: {0}")]
    InternalError(String),

    #[error("CString conversion error: {0}")]
    CStringError(#[from] std::ffi::NulError),

    #[error("OMMX error: {0}")]
    OmmxError(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, ScipAdapterError>;
