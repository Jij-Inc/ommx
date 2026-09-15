//! Construct exceptions using the Python SDK's canonical class.

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
