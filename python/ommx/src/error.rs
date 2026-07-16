//! Translation from Rust SDK errors to Python exceptions.
//!
//! Binding entry points return [`OmmxPyResult`] so `?` classifies concrete Rust
//! SDK signals through the declarative mapping table below. Signals already
//! erased into `ommx::Error` are recovered through the same table before PyO3
//! receives the local [`OmmxPyError`] wrapper.

use pyo3::{
    exceptions::{PyKeyError, PyRuntimeError, PyValueError},
    prelude::*,
};

pyo3::create_exception!(
    ommx._ommx_rust,
    RemoteArtifactError,
    PyRuntimeError,
    "Base exception for failures while accessing a remote OMMX Artifact."
);
impl pyo3_stub_gen::PyStubType for RemoteArtifactError {
    fn type_output() -> pyo3_stub_gen::TypeInfo {
        pyo3_stub_gen::TypeInfo::locally_defined("RemoteArtifactError", "ommx._ommx_rust".into())
    }
}
pyo3_stub_gen::impl_py_runtime_type!(RemoteArtifactError);
pyo3_stub_gen::inventory::submit! {
    pyo3_stub_gen::type_info::PyClassInfo {
        pyclass_name: "RemoteArtifactError",
        struct_id: std::any::TypeId::of::<RemoteArtifactError>,
        getters: &[],
        setters: &[],
        module: Some("ommx._ommx_rust"),
        doc: "Base exception for failures while accessing a remote OMMX Artifact.",
        bases: &[|| <PyRuntimeError as pyo3_stub_gen::PyStubType>::type_output()],
        has_eq: false,
        has_ord: false,
        has_hash: false,
        has_str: false,
        subclass: true,
    }
}
pyo3_stub_gen::create_exception!(
    ommx._ommx_rust,
    RemoteArtifactNotFoundError,
    RemoteArtifactError,
    "The requested remote Artifact manifest does not exist."
);
pyo3_stub_gen::create_exception!(
    ommx._ommx_rust,
    RemoteArtifactAuthenticationError,
    RemoteArtifactError,
    "Authentication for the remote Artifact registry failed."
);
pyo3_stub_gen::create_exception!(
    ommx._ommx_rust,
    RemoteArtifactAuthorizationError,
    RemoteArtifactError,
    "The caller is not authorized to read the remote Artifact."
);
pyo3_stub_gen::create_exception!(
    ommx._ommx_rust,
    RemoteArtifactTransportError,
    RemoteArtifactError,
    "The remote Artifact registry could not be reached or failed."
);
pyo3_stub_gen::create_exception!(
    ommx._ommx_rust,
    InvalidRemoteArtifactError,
    RemoteArtifactError,
    "The remote response is not a valid OMMX Artifact."
);

/// Binding-internal wrapper around an already classified Python exception.
///
/// Each Rust SDK signal declares its Python mapping below. Python-owned errors
/// pass through unchanged.
#[derive(Debug)]
pub struct OmmxPyError(PyErr);

/// Result type for Rust SDK failures crossing the private binding boundary.
pub type OmmxPyResult<T> = std::result::Result<T, OmmxPyError>;

fn value_error<T>(_: &T, message: String) -> PyErr {
    PyValueError::new_err(message)
}

#[cfg(feature = "remote-artifact")]
fn remote_artifact_error_to_pyerr(
    error: &ommx::artifact::RemoteArtifactError,
    message: String,
) -> PyErr {
    match error {
        ommx::artifact::RemoteArtifactError::ManifestNotFound { .. } => {
            RemoteArtifactNotFoundError::new_err(message)
        }
        ommx::artifact::RemoteArtifactError::Authentication { .. } => {
            RemoteArtifactAuthenticationError::new_err(message)
        }
        ommx::artifact::RemoteArtifactError::Authorization { .. } => {
            RemoteArtifactAuthorizationError::new_err(message)
        }
        ommx::artifact::RemoteArtifactError::Transport { .. } => {
            RemoteArtifactTransportError::new_err(message)
        }
        ommx::artifact::RemoteArtifactError::InvalidArtifact { .. } => {
            InvalidRemoteArtifactError::new_err(message)
        }
        ommx::artifact::RemoteArtifactError::Other { .. } => RemoteArtifactError::new_err(message),
        _ => RemoteArtifactError::new_err(message),
    }
}

/// Declare both the direct typed conversion and the fallback dispatch used
/// after a Rust SDK signal has already been erased into `ommx::Error`.
///
/// Declaration order is significant when an `anyhow::Error` can be downcast to
/// multiple mapped signals: the Python-visible owner must appear first.
macro_rules! define_ommx_error_mappings {
    (
        $(
            $(#[$attribute:meta])*
            $signal:ty => $mapper:path
        ),+ $(,)?
    ) => {
        $(
            $(#[$attribute])*
            impl From<$signal> for OmmxPyError {
                fn from(error: $signal) -> Self {
                    let message = error.to_string();
                    Self($mapper(&error, message))
                }
            }
        )+

        impl From<ommx::Error> for OmmxPyError {
            fn from(error: ommx::Error) -> Self {
                let message = format!("{error:#}");

                $(
                    $(#[$attribute])*
                    if let Some(signal) = error.downcast_ref::<$signal>() {
                        return Self($mapper(signal, message));
                    }
                )+

                Self(PyRuntimeError::new_err(message))
            }
        }
    };
}

fn invalid_local_registry_image_ref_to_pyerr(
    error: &ommx::artifact::local_registry::InvalidLocalRegistryImageRef,
    _message: String,
) -> PyErr {
    // This owner describes corrupted persisted registry state, even though its
    // source is an ImageRefParseError caused by the stored value.
    PyRuntimeError::new_err(error.to_string())
}

fn image_ref_parse_error_to_pyerr(
    error: &ommx::artifact::ImageRefParseError,
    _message: String,
) -> PyErr {
    // ImageRefParseError's Display already includes the OCI parser source.
    PyValueError::new_err(error.to_string())
}

fn parse_error_to_pyerr(error: &ommx::ParseError, _message: String) -> PyErr {
    // ParseError's Display already renders its complete traceback. Reusing the
    // anyhow chain would repeat the protobuf parser source.
    PyValueError::new_err(error.to_string())
}

fn decision_variable_error_to_pyerr(error: &ommx::DecisionVariableError, message: String) -> PyErr {
    if matches!(
        error,
        ommx::DecisionVariableError::BoundInconsistentToKind { .. }
            | ommx::DecisionVariableError::DuplicateID { .. }
            | ommx::DecisionVariableError::NoAvailableID
            | ommx::DecisionVariableError::NonFiniteValue { .. }
            | ommx::DecisionVariableError::SubstitutedValueOverwrite { .. }
            | ommx::DecisionVariableError::SubstitutedValueInconsistent { .. }
            | ommx::DecisionVariableError::EmptyBoundIntersection { .. }
    ) {
        PyValueError::new_err(message)
    } else {
        PyRuntimeError::new_err(message)
    }
}

fn solution_error_to_pyerr(error: &ommx::SolutionError, message: String) -> PyErr {
    match error {
        ommx::SolutionError::UnknownConstraintID { .. }
        | ommx::SolutionError::UnknownVariableName { .. }
        | ommx::SolutionError::UnknownConstraintName { .. }
        | ommx::SolutionError::UnknownNamedFunctionName { .. } => PyKeyError::new_err(message),
        ommx::SolutionError::ParameterizedConstraint
        | ommx::SolutionError::DuplicateSubscript { .. } => PyValueError::new_err(message),
        _ => PyRuntimeError::new_err(message),
    }
}

fn sample_set_error_to_pyerr(error: &ommx::SampleSetError, message: String) -> PyErr {
    match error {
        ommx::SampleSetError::UnknownVariableName { .. }
        | ommx::SampleSetError::UnknownConstraintName { .. }
        | ommx::SampleSetError::UnknownSampleID { .. }
        | ommx::SampleSetError::UnknownNamedFunctionName { .. } => PyKeyError::new_err(message),
        ommx::SampleSetError::DuplicateSubscripts { .. }
        | ommx::SampleSetError::ParameterizedConstraint
        | ommx::SampleSetError::NoFeasibleSolution
        | ommx::SampleSetError::NoFeasibleSolutionRelaxed => PyValueError::new_err(message),
        _ => PyRuntimeError::new_err(message),
    }
}

define_ommx_error_mappings!(
    ommx::ParseError => parse_error_to_pyerr,
    ommx::artifact::local_registry::InvalidLocalRegistryImageRef => invalid_local_registry_image_ref_to_pyerr,
    #[cfg(feature = "remote-artifact")]
    ommx::artifact::RemoteArtifactError => remote_artifact_error_to_pyerr,
    ommx::artifact::ImageRefParseError => image_ref_parse_error_to_pyerr,
    ommx::DecisionVariableError => decision_variable_error_to_pyerr,
    ommx::SolutionError => solution_error_to_pyerr,
    ommx::SampleSetError => sample_set_error_to_pyerr,
    ommx::AtolError => value_error,
    ommx::BoundError => value_error,
    ommx::CoefficientError => value_error,
    ommx::qplib::QplibParseError => value_error,
);

impl From<PyErr> for OmmxPyError {
    fn from(error: PyErr) -> Self {
        Self(error)
    }
}

impl From<OmmxPyError> for PyErr {
    fn from(OmmxPyError(error): OmmxPyError) -> Self {
        error
    }
}

/// Register the Python exception hierarchy owned by this conversion boundary.
pub fn register_exceptions(py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add("RemoteArtifactError", py.get_type::<RemoteArtifactError>())?;
    module.add(
        "RemoteArtifactNotFoundError",
        py.get_type::<RemoteArtifactNotFoundError>(),
    )?;
    module.add(
        "RemoteArtifactAuthenticationError",
        py.get_type::<RemoteArtifactAuthenticationError>(),
    )?;
    module.add(
        "RemoteArtifactAuthorizationError",
        py.get_type::<RemoteArtifactAuthorizationError>(),
    )?;
    module.add(
        "RemoteArtifactTransportError",
        py.get_type::<RemoteArtifactTransportError>(),
    )?;
    module.add(
        "InvalidRemoteArtifactError",
        py.get_type::<InvalidRemoteArtifactError>(),
    )?;
    Ok(())
}
