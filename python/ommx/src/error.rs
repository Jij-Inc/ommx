//! Translation from Rust SDK errors to Python exceptions.
//!
//! Rust SDK methods keep returning `ommx::Result<T>`. Binding entry points
//! return [`OmmxPyResult`] so `?` converts SDK errors into the local
//! [`OmmxPyError`] wrapper before PyO3 invokes this module's single type-based
//! classifier.

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

#[derive(Debug)]
enum OmmxPyErrorKind {
    Sdk(ommx::Error),
    Python(PyErr),
}

/// Binding-internal wrapper that gives PyO3 a local error conversion point.
///
/// Rust SDK errors are classified below, while Python-owned errors pass
/// through unchanged.
#[derive(Debug)]
pub struct OmmxPyError(OmmxPyErrorKind);

impl std::fmt::Display for OmmxPyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            OmmxPyErrorKind::Sdk(error) => write!(formatter, "{error:#}"),
            OmmxPyErrorKind::Python(error) => error.fmt(formatter),
        }
    }
}

/// Result type for Rust SDK failures crossing the private binding boundary.
pub type OmmxPyResult<T> = std::result::Result<T, OmmxPyError>;

impl From<ommx::Error> for OmmxPyError {
    fn from(error: ommx::Error) -> Self {
        Self(OmmxPyErrorKind::Sdk(error))
    }
}

macro_rules! impl_from_ommx_signal {
    ($($error:ty),+ $(,)?) => {
        $(
            impl From<$error> for OmmxPyError {
                fn from(error: $error) -> Self {
                    Self::from(ommx::Error::from(error))
                }
            }
        )+
    };
}

impl_from_ommx_signal!(
    ommx::AddDecisionVariableError,
    ommx::AtolError,
    ommx::BoundError,
    ommx::CoefficientError,
    ommx::ContentFactorError,
    ommx::DecisionVariableError,
    ommx::DuplicatedSampleIDError,
    ommx::EvaluationError,
    ommx::OneHotConstraintError,
    ommx::Sos1ConstraintError,
    ommx::SubstitutionError,
    ommx::random::SamplesParametersError,
    ommx::artifact::ImageRefParseError,
    ommx::experiment::AttachmentNotFound,
    ommx::ParseError,
    ommx::SampleSetError,
    ommx::SolutionError,
    ommx::qplib::QplibParseError,
);

impl From<PyErr> for OmmxPyError {
    fn from(error: PyErr) -> Self {
        Self(OmmxPyErrorKind::Python(error))
    }
}

impl From<OmmxPyError> for PyErr {
    fn from(OmmxPyError(error): OmmxPyError) -> Self {
        match error {
            OmmxPyErrorKind::Sdk(error) => ommx_error_to_pyerr(error),
            OmmxPyErrorKind::Python(error) => error,
        }
    }
}

fn ommx_error_to_pyerr(error: ommx::Error) -> PyErr {
    // ParseError may contain another mapped signal as its source. Parsing is
    // the Python-visible operation, so classify it before inspecting nested
    // domain errors. Its Display already renders the complete parse traceback.
    if let Some(parse_error) = error.downcast_ref::<ommx::ParseError>() {
        return PyValueError::new_err(parse_error.to_string());
    }

    // A nested ImageRefParseError describes corrupted persisted state here,
    // not the caller's current input. Preserve the Local Registry owner before
    // classifying the underlying parser signal.
    if let Some(registry_error) =
        error.downcast_ref::<ommx::artifact::local_registry::InvalidLocalRegistryImageRef>()
    {
        return PyRuntimeError::new_err(registry_error.to_string());
    }

    // ImageRefParseError's Display already includes its source. Rendering the
    // complete anyhow chain would repeat the OCI parser message.
    if let Some(image_ref_error) = error.downcast_ref::<ommx::artifact::ImageRefParseError>() {
        return PyValueError::new_err(image_ref_error.to_string());
    }

    if let Some(attachment_error) = error.downcast_ref::<ommx::experiment::AttachmentNotFound>() {
        return PyKeyError::new_err(attachment_error.name().to_string());
    }

    let message = format!("{error:#}");

    if error
        .downcast_ref::<ommx::qplib::QplibParseError>()
        .is_some()
    {
        return PyValueError::new_err(message);
    }

    if error.downcast_ref::<ommx::CoefficientError>().is_some()
        || error
            .downcast_ref::<ommx::AddDecisionVariableError>()
            .is_some()
        || error.downcast_ref::<ommx::AtolError>().is_some()
        || error.downcast_ref::<ommx::BoundError>().is_some()
        || error.downcast_ref::<ommx::ContentFactorError>().is_some()
        || error
            .downcast_ref::<ommx::DuplicatedSampleIDError>()
            .is_some()
        || error.downcast_ref::<ommx::EvaluationError>().is_some()
        || error
            .downcast_ref::<ommx::OneHotConstraintError>()
            .is_some()
        || error.downcast_ref::<ommx::Sos1ConstraintError>().is_some()
        || error.downcast_ref::<ommx::SubstitutionError>().is_some()
        || error
            .downcast_ref::<ommx::random::SamplesParametersError>()
            .is_some()
    {
        return PyValueError::new_err(message);
    }

    if let Some(error) = error.downcast_ref::<ommx::DecisionVariableError>() {
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
            return PyValueError::new_err(message);
        }
    }

    if let Some(error) = error.downcast_ref::<ommx::SolutionError>() {
        match error {
            ommx::SolutionError::UnknownConstraintID { .. }
            | ommx::SolutionError::UnknownVariableName { .. }
            | ommx::SolutionError::UnknownConstraintName { .. }
            | ommx::SolutionError::UnknownNamedFunctionName { .. } => {
                return PyKeyError::new_err(message);
            }
            ommx::SolutionError::ParameterizedConstraint
            | ommx::SolutionError::DuplicateSubscript { .. } => {
                return PyValueError::new_err(message);
            }
            _ => {}
        }
    }

    if let Some(error) = error.downcast_ref::<ommx::SampleSetError>() {
        match error {
            ommx::SampleSetError::UnknownVariableName { .. }
            | ommx::SampleSetError::UnknownConstraintName { .. }
            | ommx::SampleSetError::UnknownSampleID { .. }
            | ommx::SampleSetError::UnknownNamedFunctionName { .. } => {
                return PyKeyError::new_err(message);
            }
            ommx::SampleSetError::DuplicateSubscripts { .. }
            | ommx::SampleSetError::ParameterizedConstraint
            | ommx::SampleSetError::NoFeasibleSolution
            | ommx::SampleSetError::NoFeasibleSolutionRelaxed => {
                return PyValueError::new_err(message);
            }
            _ => {}
        }
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
