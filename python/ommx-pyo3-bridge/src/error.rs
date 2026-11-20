//! Construct exceptions using the Python SDK's canonical class.

use crate::TransferProtocolId;
use pyo3::{
    exceptions::{PyBaseException, PyImportError, PyTypeError},
    prelude::*,
    types::PyType,
};

/// Constructors for the installed Python SDK's `ommx.BridgeError` exception.
///
/// The Python SDK defines the class and supplies it to receiver registration.
/// Senders retrieve that Python class, so they never define a competing type
/// in their independently built extension.
pub struct BridgeError;

impl BridgeError {
    /// Get the exception class published by the installed Python SDK.
    /// A missing SDK or bridge class raises `ImportError`.
    pub fn type_object(py: Python<'_>) -> PyResult<Bound<'_, PyType>> {
        Self::from_module(&PyModule::import(py, "ommx")?)
    }

    /// Create an SDK-owned bridge exception, for example when a protocol is unsupported.
    /// A missing SDK or bridge class raises `ImportError`.
    pub fn new_err(py: Python<'_>, message: impl Into<String>) -> PyErr {
        match Self::type_object(py) {
            Ok(class) => PyErr::from_type(class, message.into()),
            Err(error) => error,
        }
    }

    /// Report that none of the caller's requested protocols is supported.
    ///
    /// Call this after `resolve_target` (the `sender` feature) returns `None` for every
    /// acceptable protocol. The diagnostic lists the requested protocols in
    /// caller order; this constructor does not probe support or retry transfers.
    /// A missing SDK or bridge exception class raises `ImportError`.
    pub fn no_supported_protocol(py: Python<'_>, requested: &[TransferProtocolId]) -> PyErr {
        let message = match requested {
            [] => "No OMMX transfer protocols were requested".to_owned(),
            [protocol] => format!(
                "The loaded OMMX Python SDK does not support the {protocol} transfer protocol"
            ),
            protocols => {
                let names = protocols
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "The loaded OMMX Python SDK does not support any requested transfer protocol: {names}"
                )
            }
        };
        Self::new_err(py, message)
    }

    // Protocol discovery retains the class from the same SDK as the receivers.
    pub(crate) fn from_module<'py>(module: &Bound<'py, PyModule>) -> PyResult<Bound<'py, PyType>> {
        module
            .getattr("_ommx_rust")
            .and_then(|module| module.getattr(crate::protocol::BRIDGE_ERROR))
            .and_then(|class| class.cast_into::<PyType>().map_err(Into::into))
            .and_then(|class| {
                if class.is_subclass_of::<PyBaseException>()? {
                    Ok(class)
                } else {
                    Err(PyTypeError::new_err(
                        "BridgeError must be a Python exception class",
                    ))
                }
            })
            .map_err(|source| {
                let error = PyImportError::new_err(
                    "The OMMX Python SDK must expose ommx._ommx_rust.BridgeError",
                );
                error.set_cause(module.py(), Some(source));
                error
            })
    }

    // Sender error conversion keeps the SDK class and original Python cause.
    #[cfg(feature = "sender")]
    pub(crate) fn with_cause(
        class: &Bound<'_, PyType>,
        message: impl Into<String>,
        source: PyErr,
    ) -> PyErr {
        let error = PyErr::from_type(class.clone(), message.into());
        error.set_cause(class.py(), Some(source));
        error
    }
}
