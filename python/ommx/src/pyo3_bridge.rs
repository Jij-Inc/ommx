//! Register the V1 wire contract; Python factories own the legacy wrapper classes.

use ommx_pyo3_bridge::{register_receivers, ProtobufV1ReceiverConfig};
use pyo3::{
    prelude::*,
    types::{PyBytes, PyType},
};

fn receive(py: Python<'_>, bytes: &[u8], factory: &str) -> PyResult<Py<PyAny>> {
    Ok(py
        .import("ommx._bridge")?
        .getattr(factory)?
        .call1((PyBytes::new(py, bytes),))?
        .unbind())
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let error = module
        .py()
        .import("ommx._bridge_error")?
        .getattr("BridgeError")?
        .cast_into::<PyType>()?;
    register_receivers(
        module,
        &error,
        [ProtobufV1ReceiverConfig {
            function: |py, bytes| receive(py, bytes, "function"),
            constraint: |py, bytes| receive(py, bytes, "constraint"),
            decision_variable: |py, bytes| receive(py, bytes, "decision_variable"),
            instance: |py, bytes| receive(py, bytes, "instance"),
            parametric_instance: |py, bytes| receive(py, bytes, "parametric_instance"),
            solution: |py, bytes| receive(py, bytes, "solution"),
            sample_set: |py, bytes| receive(py, bytes, "sample_set"),
        }
        .into()],
    )
}
