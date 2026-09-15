//! ProtobufV2 receiver configuration and Python call signatures.

use super::{bind_methods, protobuf, Binding};
use crate::protocol;
use pyo3::{exceptions::PyRuntimeError, prelude::*, types::PyBytes};

/// Python SDK factories for the complete ProtobufV2 transfer contract.
///
/// Register with [`super::register_receivers`]. This type fixes the protocol ID,
/// supported types, payloads, and parser selection. All factories are required
/// and must preserve the parsed domain data when constructing the SDK's
/// canonical Python classes. They run inside the receiving extension; no Rust
/// value or factory pointer crosses the shared-library boundary.
///
/// ProtobufV2 supplies parsed Rust SDK values. Other protocol configurations
/// may define different factory inputs and ownership rules.
pub struct ProtobufV2ReceiverConfig {
    /// Construct the SDK's canonical `ommx.Function`.
    pub function: fn(Python<'_>, ommx::Function) -> PyResult<Py<PyAny>>,
    /// Construct its detached `ommx.Constraint`, preserving the complete context.
    pub constraint:
        fn(Python<'_>, ommx::Constraint, ommx::ConstraintContext) -> PyResult<Py<PyAny>>,
    /// Construct its detached `ommx.DecisionVariable`, preserving its ID and label.
    pub decision_variable: fn(
        Python<'_>,
        ommx::VariableID,
        ommx::DecisionVariable,
        ommx::ModelingLabel,
    ) -> PyResult<Py<PyAny>>,
    /// Construct `ommx.Instance` with all root-owned data.
    pub instance: fn(Python<'_>, ommx::Instance) -> PyResult<Py<PyAny>>,
    /// Construct `ommx.ParametricInstance` with all root-owned data.
    pub parametric_instance: fn(Python<'_>, ommx::ParametricInstance) -> PyResult<Py<PyAny>>,
    /// Construct `ommx.Solution` with all evaluated data.
    pub solution: fn(Python<'_>, ommx::Solution) -> PyResult<Py<PyAny>>,
    /// Construct `ommx.SampleSet` with all sampled data.
    pub sample_set: fn(Python<'_>, ommx::SampleSet) -> PyResult<Py<PyAny>>,
}

pub fn bindings(config: ProtobufV2ReceiverConfig, py: Python<'_>) -> PyResult<Vec<Binding>> {
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

fn parse_error(error: ommx::Error) -> PyErr {
    PyRuntimeError::new_err(format!("invalid OMMX ProtobufV2 bridge payload: {error:#}"))
}

#[pymethods]
impl Receiver {
    fn function(&self, bytes: &Bound<'_, PyBytes>) -> PyResult<Py<PyAny>> {
        let value = protobuf::function(bytes.as_bytes()).map_err(parse_error)?;
        (self.config.function)(bytes.py(), value)
    }

    fn constraint(
        &self,
        constraint: &Bound<'_, PyBytes>,
        context: &Bound<'_, PyBytes>,
    ) -> PyResult<Py<PyAny>> {
        let (value, context) = protobuf::constraint_v2(constraint.as_bytes(), context.as_bytes())
            .map_err(parse_error)?;
        (self.config.constraint)(constraint.py(), value, context)
    }

    fn decision_variable(
        &self,
        id: u64,
        decision_variable: &Bound<'_, PyBytes>,
        label: &Bound<'_, PyBytes>,
    ) -> PyResult<Py<PyAny>> {
        let (id, value, label) =
            protobuf::decision_variable_v2(id, decision_variable.as_bytes(), label.as_bytes())
                .map_err(parse_error)?;
        (self.config.decision_variable)(decision_variable.py(), id, value, label)
    }

    fn instance(&self, bytes: &Bound<'_, PyBytes>) -> PyResult<Py<PyAny>> {
        let value = ommx::Instance::from_v2_bytes(bytes.as_bytes()).map_err(parse_error)?;
        (self.config.instance)(bytes.py(), value)
    }

    fn parametric_instance(&self, bytes: &Bound<'_, PyBytes>) -> PyResult<Py<PyAny>> {
        let value =
            ommx::ParametricInstance::from_v2_bytes(bytes.as_bytes()).map_err(parse_error)?;
        (self.config.parametric_instance)(bytes.py(), value)
    }

    fn solution(&self, bytes: &Bound<'_, PyBytes>) -> PyResult<Py<PyAny>> {
        let value = ommx::Solution::from_v2_bytes(bytes.as_bytes()).map_err(parse_error)?;
        (self.config.solution)(bytes.py(), value)
    }

    fn sample_set(&self, bytes: &Bound<'_, PyBytes>) -> PyResult<Py<PyAny>> {
        let value = ommx::SampleSet::from_v2_bytes(bytes.as_bytes()).map_err(parse_error)?;
        (self.config.sample_set)(bytes.py(), value)
    }
}
