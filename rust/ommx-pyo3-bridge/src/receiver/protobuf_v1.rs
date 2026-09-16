//! ProtobufV1 receiver configuration and Python call signatures.

use super::{bind_methods, Binding};
use crate::protocol;
use pyo3::{prelude::*, types::PyBytes};

/// Python SDK factories for the complete ProtobufV1 transfer contract.
///
/// Register with [`super::register_receivers`]. This type fixes the protocol ID,
/// supported types, and wire arguments, without depending on a Rust SDK version.
/// All factories are required. Each factory must parse and validate its payload
/// using the receiving SDK's parser, then construct the canonical Python class.
/// Preserve all data required by the protocol, including owner-side context.
/// Unsupported features must be rejected by the parser rather than silently lost.
///
/// Byte slices are borrowed for the duration of the call and are passed unchanged.
/// Factories own the returned Python objects. No Rust value or factory pointer
/// crosses the shared-library boundary. Factory errors propagate unchanged; the
/// SDK maps payload errors to its `BridgeError`, while Python-owned errors keep
/// their original classification. A sender adds transfer context and a cause.
/// Other protocol configurations may define different inputs and ownership rules.
pub struct ProtobufV1ReceiverConfig {
    /// Parse an `ommx.v1.Function` and construct `ommx.Function`.
    pub function: fn(Python<'_>, &[u8]) -> PyResult<Py<PyAny>>,
    /// Parse a complete `ommx.v1.Constraint`, including its ID and label.
    pub constraint: fn(Python<'_>, &[u8]) -> PyResult<Py<PyAny>>,
    /// Parse a complete `ommx.v1.DecisionVariable`, including its ID and label.
    pub decision_variable: fn(Python<'_>, &[u8]) -> PyResult<Py<PyAny>>,
    /// Parse an `ommx.v1.Instance` and construct `ommx.Instance`.
    pub instance: fn(Python<'_>, &[u8]) -> PyResult<Py<PyAny>>,
    /// Parse an `ommx.v1.ParametricInstance` and construct `ommx.ParametricInstance`.
    pub parametric_instance: fn(Python<'_>, &[u8]) -> PyResult<Py<PyAny>>,
    /// Parse an `ommx.v1.Solution` and construct `ommx.Solution`.
    pub solution: fn(Python<'_>, &[u8]) -> PyResult<Py<PyAny>>,
    /// Parse an `ommx.v1.SampleSet` and construct `ommx.SampleSet`.
    pub sample_set: fn(Python<'_>, &[u8]) -> PyResult<Py<PyAny>>,
}

pub fn bindings(py: Python<'_>, config: ProtobufV1ReceiverConfig) -> PyResult<Vec<Binding>> {
    let receiver = Py::new(py, Receiver { config })?.into_bound(py).into_any();
    bind_methods(
        receiver,
        &[
            (protocol::V1_FUNCTION, "function"),
            (protocol::V1_CONSTRAINT, "constraint"),
            (protocol::V1_DECISION_VARIABLE, "decision_variable"),
            (protocol::V1_INSTANCE, "instance"),
            (protocol::V1_PARAMETRIC_INSTANCE, "parametric_instance"),
            (protocol::V1_SOLUTION, "solution"),
            (protocol::V1_SAMPLE_SET, "sample_set"),
        ],
    )
}

// This implementation type stays local to the receiving shared library.
#[pyclass(frozen, module = "ommx._ommx_rust")]
struct Receiver {
    config: ProtobufV1ReceiverConfig,
}

#[pymethods]
impl Receiver {
    fn function(&self, bytes: &Bound<'_, PyBytes>) -> PyResult<Py<PyAny>> {
        (self.config.function)(bytes.py(), bytes.as_bytes())
    }

    fn constraint(&self, bytes: &Bound<'_, PyBytes>) -> PyResult<Py<PyAny>> {
        (self.config.constraint)(bytes.py(), bytes.as_bytes())
    }

    fn decision_variable(&self, bytes: &Bound<'_, PyBytes>) -> PyResult<Py<PyAny>> {
        (self.config.decision_variable)(bytes.py(), bytes.as_bytes())
    }

    fn instance(&self, bytes: &Bound<'_, PyBytes>) -> PyResult<Py<PyAny>> {
        (self.config.instance)(bytes.py(), bytes.as_bytes())
    }

    fn parametric_instance(&self, bytes: &Bound<'_, PyBytes>) -> PyResult<Py<PyAny>> {
        (self.config.parametric_instance)(bytes.py(), bytes.as_bytes())
    }

    fn solution(&self, bytes: &Bound<'_, PyBytes>) -> PyResult<Py<PyAny>> {
        (self.config.solution)(bytes.py(), bytes.as_bytes())
    }

    fn sample_set(&self, bytes: &Bound<'_, PyBytes>) -> PyResult<Py<PyAny>> {
        (self.config.sample_set)(bytes.py(), bytes.as_bytes())
    }
}
