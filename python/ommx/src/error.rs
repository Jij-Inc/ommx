//! Translation from Rust SDK errors to Python exceptions.
//!
//! Rust SDK methods keep returning `ommx::Result<T>`. Binding entry points
//! route those operations through [`map_ommx_error`], which converts the
//! erased `ommx::Error` with this module's single type-based classifier.

use pyo3::{
    exceptions::{PyRuntimeError, PyValueError},
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

/// Binding-internal wrapper that gives PyO3 a local error conversion point.
#[derive(Debug)]
struct OmmxPyError(ommx::Error);

/// Intermediate result kept private inside the binding error boundary.
type OmmxPyResult<T> = std::result::Result<T, OmmxPyError>;

impl From<ommx::Error> for OmmxPyError {
    fn from(error: ommx::Error) -> Self {
        Self(error)
    }
}

impl From<OmmxPyError> for PyErr {
    fn from(OmmxPyError(error): OmmxPyError) -> Self {
        ommx_error_to_pyerr(error)
    }
}

fn ommx_error_to_pyerr(error: ommx::Error) -> PyErr {
    let message = format!("{error:#}");

    if error.downcast_ref::<ommx::CoefficientError>().is_some()
        || error.downcast_ref::<ommx::AtolError>().is_some()
        || error.downcast_ref::<ommx::BoundError>().is_some()
    {
        return PyValueError::new_err(message);
    }

    #[cfg(feature = "remote-artifact")]
    if let Some(remote_error) = error.downcast_ref::<ommx::artifact::RemoteArtifactError>() {
        return match remote_error {
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
            ommx::artifact::RemoteArtifactError::Other { .. } => {
                RemoteArtifactError::new_err(message)
            }
            _ => RemoteArtifactError::new_err(message),
        };
    }

    PyRuntimeError::new_err(message)
}

/// Run a Rust SDK operation through the binding-owned Python error mapper.
///
/// This function is public only inside the private `error` module boundary so
/// sibling binding modules can share the conversion without exposing its
/// wrapper type in their public PyO3 method signatures.
pub fn map_ommx_error<T>(operation: impl FnOnce() -> ommx::Result<T>) -> PyResult<T> {
    let result: OmmxPyResult<T> = operation().map_err(Into::into);
    result.map_err(Into::into)
}

/// Route a typed coefficient result through the shared SDK error classifier.
pub fn map_coefficient<T>(result: std::result::Result<T, ommx::CoefficientError>) -> PyResult<T> {
    map_ommx_error(|| Ok(result?))
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
