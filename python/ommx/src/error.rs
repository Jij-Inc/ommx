//! Translation from Rust SDK errors to Python exceptions.
//!
//! Binding entry points return [`OmmxPyResult`] so `?` classifies concrete Rust
//! SDK signals through the declarative mapping table below. Signals already
//! erased into `ommx::Error` are recovered through the same table before PyO3
//! receives the local [`OmmxPyError`] wrapper. Binding-owned Rust errors that
//! are not SDK signals use direct `From` implementations beside that table.

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

/// Define a core-owned exception whose public runtime module is `ommx` while
/// keeping its generated binding stub in the private extension module.
macro_rules! create_core_exception {
    ($name:ident, $base:ty, $doc:expr) => {
        pyo3::create_exception!(ommx, $name, $base, $doc);

        impl pyo3_stub_gen::PyStubType for $name {
            fn type_output() -> pyo3_stub_gen::TypeInfo {
                pyo3_stub_gen::TypeInfo::builtin(stringify!($name))
            }
        }

        pyo3_stub_gen::impl_py_runtime_type!($name);

        pyo3_stub_gen::inventory::submit! {
            pyo3_stub_gen::type_info::PyClassInfo {
                pyclass_name: stringify!($name),
                struct_id: std::any::TypeId::of::<$name>,
                getters: &[],
                setters: &[],
                module: Some("ommx._ommx_rust"),
                doc: $doc,
                bases: &[|| <$base as pyo3_stub_gen::PyStubType>::type_output()],
                has_eq: false,
                has_ord: false,
                has_hash: false,
                has_str: false,
                subclass: true,
            }
        }
    };
}

pyo3::create_exception!(
    ommx,
    LogEncodingError,
    PyRuntimeError,
    "An exact log encoding is unavailable for one requested decision variable. Diagnostic attributes are ``kind``, ``variable_id``, ``observed``, and ``expected``."
);
impl pyo3_stub_gen::PyStubType for LogEncodingError {
    fn type_output() -> pyo3_stub_gen::TypeInfo {
        pyo3_stub_gen::TypeInfo::builtin("LogEncodingError")
    }
}
pyo3_stub_gen::impl_py_runtime_type!(LogEncodingError);

fn log_encoding_diagnostic_value_type() -> pyo3_stub_gen::TypeInfo {
    use pyo3_stub_gen::PyStubType;
    String::type_output() | u64::type_output() | f64::type_output()
}

pyo3_stub_gen::inventory::submit! {
    pyo3_stub_gen::type_info::PyClassInfo {
        pyclass_name: "LogEncodingError",
        struct_id: std::any::TypeId::of::<LogEncodingError>,
        getters: &[
            pyo3_stub_gen::type_info::MemberInfo {
                name: "kind",
                r#type: <String as pyo3_stub_gen::PyStubType>::type_output,
                doc: "Machine-readable reason for unavailable exact encoding.",
                default: None,
                deprecated: None,
            },
            pyo3_stub_gen::type_info::MemberInfo {
                name: "variable_id",
                r#type: <u64 as pyo3_stub_gen::PyStubType>::type_output,
                doc: "Decision variable for which exact encoding is unavailable.",
                default: None,
                deprecated: None,
            },
            pyo3_stub_gen::type_info::MemberInfo {
                name: "observed",
                r#type: log_encoding_diagnostic_value_type,
                doc: "Observed bound or bit count that made encoding unavailable.",
                default: None,
                deprecated: None,
            },
            pyo3_stub_gen::type_info::MemberInfo {
                name: "expected",
                r#type: log_encoding_diagnostic_value_type,
                doc: "Required bound condition or maximum bit count.",
                default: None,
                deprecated: None,
            },
        ],
        setters: &[],
        // Runtime ownership and generated-stub placement are intentionally
        // separate: the public type is `ommx.LogEncodingError`.
        module: Some("ommx._ommx_rust"),
        doc: "An exact log encoding is unavailable for one requested decision variable.",
        bases: &[|| <PyRuntimeError as pyo3_stub_gen::PyStubType>::type_output()],
        has_eq: false,
        has_ord: false,
        has_hash: false,
        has_str: false,
        subclass: true,
    }
}
create_core_exception!(
    ExactIntegerSlackError,
    PyRuntimeError,
    "Exact integer-slack conversion is unavailable for the requested inequality."
);
create_core_exception!(
    InfeasibleDetected,
    PyRuntimeError,
    "The mathematical model was proven infeasible."
);

/// Binding-internal wrapper around an already classified Python exception.
///
/// Each Rust SDK signal declares its Python mapping below. Python-owned errors
/// pass through unchanged.
#[derive(Debug)]
pub struct OmmxPyError(PyErr);

impl std::fmt::Display for OmmxPyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Result type for Rust-owned failures crossing the private binding boundary.
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

fn attachment_not_found_to_pyerr(
    error: &ommx::experiment::AttachmentNotFound,
    _message: String,
) -> PyErr {
    PyKeyError::new_err(error.name().to_string())
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
        | ommx::SolutionError::UnknownNamedFunctionID { .. }
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

fn log_encoding_unavailable_to_pyerr(
    error: &ommx::LogEncodingUnavailable,
    message: String,
) -> PyErr {
    let pyerr = LogEncodingError::new_err(message);
    Python::attach(|py| {
        let value = pyerr.value(py);
        let (kind, variable_id) = match error {
            ommx::LogEncodingUnavailable::NonFiniteBound { id, bound } => {
                value.setattr(
                    "observed",
                    format!("[{}, {}]", bound.lower(), bound.upper()),
                )?;
                value.setattr("expected", "finite integer range")?;
                ("non_finite_bound", *id)
            }
            ommx::LogEncodingUnavailable::RangeOutsideExactIntegerDomain {
                id,
                lower,
                upper,
                max_abs,
            } => {
                let observed = if *lower < -*max_abs { *lower } else { *upper };
                value.setattr("observed", observed)?;
                value.setattr("expected", *max_abs)?;
                ("outside_exact_integer_domain", *id)
            }
            ommx::LogEncodingUnavailable::RangeTooLarge {
                id,
                required_bits,
                max_bits,
            } => {
                value.setattr("observed", *required_bits)?;
                value.setattr("expected", *max_bits)?;
                ("range_too_large", *id)
            }
            _ => ("unavailable", error.variable_id()),
        };
        value.setattr("kind", kind)?;
        value.setattr("variable_id", variable_id.into_inner())
    })
    .expect("LogEncodingError supports diagnostic attributes");
    pyerr
}

fn exact_integer_slack_unavailable_to_pyerr(
    _: &ommx::ExactIntegerSlackUnavailable,
    message: String,
) -> PyErr {
    ExactIntegerSlackError::new_err(message)
}

fn infeasible_detected_to_pyerr(_: &ommx::InfeasibleDetected, message: String) -> PyErr {
    InfeasibleDetected::new_err(message)
}

define_ommx_error_mappings!(
    ommx::ParseError => parse_error_to_pyerr,
    ommx::artifact::local_registry::InvalidLocalRegistryImageRef => invalid_local_registry_image_ref_to_pyerr,
    ommx::experiment::AttachmentNotFound => attachment_not_found_to_pyerr,
    ommx::DecisionVariableError => decision_variable_error_to_pyerr,
    ommx::SolutionError => solution_error_to_pyerr,
    ommx::SampleSetError => sample_set_error_to_pyerr,
    ommx::LogEncodingUnavailable => log_encoding_unavailable_to_pyerr,
    ommx::ExactIntegerSlackUnavailable => exact_integer_slack_unavailable_to_pyerr,
    ommx::InfeasibleDetected => infeasible_detected_to_pyerr,
    #[cfg(feature = "remote-artifact")]
    ommx::artifact::RemoteArtifactError => remote_artifact_error_to_pyerr,
    ommx::artifact::ImageRefParseError => image_ref_parse_error_to_pyerr,
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

impl From<serde_json::Error> for OmmxPyError {
    fn from(error: serde_json::Error) -> Self {
        // Raw serde_json failures default to RuntimeError. Boundaries parsing
        // caller-provided JSON must override this with ValueError explicitly.
        Self(PyRuntimeError::new_err(error.to_string()))
    }
}

impl From<serde_pyobject::Error> for OmmxPyError {
    fn from(error: serde_pyobject::Error) -> Self {
        Self(error.into())
    }
}

impl<'a, 'py> From<pyo3::CastError<'a, 'py>> for OmmxPyError {
    fn from(error: pyo3::CastError<'a, 'py>) -> Self {
        Self(error.into())
    }
}

impl<'py> From<pyo3::CastIntoError<'py>> for OmmxPyError {
    fn from(error: pyo3::CastIntoError<'py>) -> Self {
        Self(error.into())
    }
}

impl<T> From<std::sync::PoisonError<T>> for OmmxPyError {
    fn from(_: std::sync::PoisonError<T>) -> Self {
        Self(PyRuntimeError::new_err("Cannot get lock for RNG"))
    }
}

impl From<OmmxPyError> for PyErr {
    fn from(OmmxPyError(error): OmmxPyError) -> Self {
        error
    }
}

/// Register the Python exception hierarchy owned by this conversion boundary.
pub fn register_exceptions(py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add("LogEncodingError", py.get_type::<LogEncodingError>())?;
    module.add(
        "ExactIntegerSlackError",
        py.get_type::<ExactIntegerSlackError>(),
    )?;
    module.add("InfeasibleDetected", py.get_type::<InfeasibleDetected>())?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use pyo3::type_object::PyTypeInfo;

    fn assert_exception<T>(error: OmmxPyError)
    where
        T: PyTypeInfo,
    {
        Python::initialize();
        Python::attach(|py| {
            let error: PyErr = error.into();
            assert!(error.is_instance_of::<T>(py), "{error}");
        });
    }

    #[test]
    fn solution_error_mapping_is_shared_by_direct_and_erased_conversions() {
        assert_exception::<PyKeyError>(
            ommx::SolutionError::UnknownNamedFunctionID {
                id: ommx::NamedFunctionID::from(1),
            }
            .into(),
        );
        assert_exception::<PyKeyError>(
            ommx::Error::from(ommx::SolutionError::UnknownNamedFunctionID {
                id: ommx::NamedFunctionID::from(1),
            })
            .into(),
        );

        assert_exception::<PyValueError>(
            ommx::SolutionError::DuplicateSubscript {
                subscripts: vec![1],
            }
            .into(),
        );
        assert_exception::<PyValueError>(
            ommx::Error::from(ommx::SolutionError::DuplicateSubscript {
                subscripts: vec![1],
            })
            .into(),
        );

        assert_exception::<PyRuntimeError>(
            ommx::SolutionError::MissingRequiredField { field: "objective" }.into(),
        );
        assert_exception::<PyRuntimeError>(
            ommx::Error::from(ommx::SolutionError::MissingRequiredField { field: "objective" })
                .into(),
        );
    }

    #[test]
    fn log_encoding_unavailable_maps_to_a_specific_runtime_error() {
        let mut instance = ommx::Instance::default();
        let id = ommx::VariableID::from(7);
        instance
            .add_decision_variable(
                id,
                ommx::DecisionVariable::integer(),
                ommx::ModelingLabel::default(),
            )
            .unwrap();
        let signal = instance
            .log_encode([id], ommx::ATol::default())
            .unwrap_err()
            .downcast::<ommx::LogEncodingUnavailable>()
            .unwrap();
        assert_exception::<LogEncodingError>(signal.clone().into());
        assert_exception::<LogEncodingError>(ommx::Error::from(signal.clone()).into());
        assert_exception::<PyRuntimeError>(signal.clone().into());

        let error: PyErr = OmmxPyError::from(signal).into();
        Python::attach(|py| {
            let value = error.value(py);
            assert_eq!(
                value.getattr("kind").unwrap().extract::<String>().unwrap(),
                "non_finite_bound"
            );
            assert_eq!(
                value
                    .getattr("variable_id")
                    .unwrap()
                    .extract::<u64>()
                    .unwrap(),
                7
            );
            assert_eq!(
                value
                    .getattr("expected")
                    .unwrap()
                    .extract::<String>()
                    .unwrap(),
                "finite integer range"
            );
        });
    }

    #[test]
    fn infeasible_detected_maps_to_a_specific_runtime_error() {
        let signal = ommx::InfeasibleDetected::InequalityConstraintBound {
            id: ommx::ConstraintID::from(3),
            bound: ommx::Bound::new(1.0, 2.0).unwrap(),
        };
        assert_exception::<InfeasibleDetected>(signal.into());

        let erased = ommx::Error::from(ommx::InfeasibleDetected::InequalityConstraintBound {
            id: ommx::ConstraintID::from(3),
            bound: ommx::Bound::new(1.0, 2.0).unwrap(),
        });
        assert_exception::<InfeasibleDetected>(erased.into());
    }

    #[test]
    fn serde_json_serialization_errors_map_to_runtime_error() {
        let value = std::collections::BTreeMap::from([(vec![1_u64], 1_u64)]);
        let error = serde_json::to_string(&value).expect_err("non-string JSON object key");
        assert_exception::<PyRuntimeError>(error.into());
    }

    #[test]
    fn serde_pyobject_errors_preserve_the_python_exception() {
        Python::initialize();
        Python::attach(|py| {
            let original = PyValueError::new_err("sentinel serde-pyobject error");
            let error = serde_pyobject::Error(original.clone_ref(py));
            let converted: PyErr = OmmxPyError::from(error).into();

            assert!(converted.value(py).is(original.value(py)));
        });
    }

    #[test]
    fn poisoned_rng_locks_map_to_runtime_error() {
        let error = std::sync::PoisonError::new(());
        assert_exception::<PyRuntimeError>(error.into());
    }
}
