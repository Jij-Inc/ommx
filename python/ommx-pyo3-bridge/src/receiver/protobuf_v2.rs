//! ProtobufV2 receiver configuration and Python call signatures.

use super::{bind_methods, Binding};
use crate::protocol;
use pyo3::{prelude::*, types::PyBytes};

/// Python SDK factories for the complete ProtobufV2 transfer contract.
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
pub struct ProtobufV2ReceiverConfig {
    /// Parse an `ommx.v1.Function` and construct `ommx.Function`.
    pub function: fn(Python<'_>, &[u8]) -> PyResult<Py<PyAny>>,
    /// Parse an `ommx.v2.RegularConstraint` and its `ommx.v2.ConstraintContext`.
    // Keep the protocol's wire arguments explicit in the configuration API.
    #[allow(clippy::type_complexity)]
    pub constraint: fn(Python<'_>, &[u8], &[u8]) -> PyResult<Py<PyAny>>,
    /// Parse the variable and label with their owner-supplied ID.
    #[allow(clippy::type_complexity)]
    pub decision_variable: fn(Python<'_>, u64, &[u8], &[u8]) -> PyResult<Py<PyAny>>,
    /// Parse an `ommx.v2.Instance` and construct `ommx.Instance`.
    pub instance: fn(Python<'_>, &[u8]) -> PyResult<Py<PyAny>>,
    /// Parse an `ommx.v2.ParametricInstance` and construct `ommx.ParametricInstance`.
    pub parametric_instance: fn(Python<'_>, &[u8]) -> PyResult<Py<PyAny>>,
    /// Parse an `ommx.v2.Solution` and construct `ommx.Solution`.
    pub solution: fn(Python<'_>, &[u8]) -> PyResult<Py<PyAny>>,
    /// Parse an `ommx.v2.SampleSet` and construct `ommx.SampleSet`.
    pub sample_set: fn(Python<'_>, &[u8]) -> PyResult<Py<PyAny>>,
}

pub fn bindings(py: Python<'_>, config: ProtobufV2ReceiverConfig) -> PyResult<Vec<Binding>> {
    let receiver = Py::new(py, Receiver { config })?.into_bound(py).into_any();
    bind_methods(
        receiver,
        &[
            (protocol::V2_FUNCTION, "function"),
            (protocol::V2_CONSTRAINT, "constraint"),
            (protocol::V2_DECISION_VARIABLE, "decision_variable"),
            (protocol::V2_INSTANCE, "instance"),
            (protocol::V2_PARAMETRIC_INSTANCE, "parametric_instance"),
            (protocol::V2_SOLUTION, "solution"),
            (protocol::V2_SAMPLE_SET, "sample_set"),
        ],
    )
}

// This implementation type stays local to the receiving shared library.
#[pyclass(frozen, module = "ommx._ommx_rust")]
struct Receiver {
    config: ProtobufV2ReceiverConfig,
}

#[pymethods]
impl Receiver {
    fn function(&self, bytes: &Bound<'_, PyBytes>) -> PyResult<Py<PyAny>> {
        (self.config.function)(bytes.py(), bytes.as_bytes())
    }

    fn constraint(
        &self,
        constraint: &Bound<'_, PyBytes>,
        context: &Bound<'_, PyBytes>,
    ) -> PyResult<Py<PyAny>> {
        (self.config.constraint)(constraint.py(), constraint.as_bytes(), context.as_bytes())
    }

    fn decision_variable(
        &self,
        id: u64,
        decision_variable: &Bound<'_, PyBytes>,
        label: &Bound<'_, PyBytes>,
    ) -> PyResult<Py<PyAny>> {
        (self.config.decision_variable)(
            decision_variable.py(),
            id,
            decision_variable.as_bytes(),
            label.as_bytes(),
        )
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
