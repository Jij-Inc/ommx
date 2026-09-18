//! An independent model producer using the published Rust SDK and V1 bridge.

mod model;

use ommx_pyo3_bridge::{resolve_target, BridgeError, ProtobufV1, PyInstance, TransferProtocolId};
use pyo3::{exceptions::PyValueError, prelude::*};

fn compile(
    py: Python<'_>,
    build: impl FnOnce() -> anyhow::Result<ommx::v1::Instance>,
) -> PyResult<PyInstance> {
    let target = resolve_target::<ProtobufV1>(py)?
        .ok_or_else(|| BridgeError::no_supported_protocol(py, &[TransferProtocolId::ProtobufV1]))?;
    let message = build().map_err(|error| PyValueError::new_err(format!("{error:#}")))?;
    target.transfer(py, message)
}

/// Minimize the given costs while choosing exactly one binary variable.
///
/// Original variable IDs are 0..len(costs); the equality has ID 23.
#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
fn compile_one_hot(py: Python<'_>, costs: Vec<f64>) -> PyResult<PyInstance> {
    compile(py, || model::one_hot(&costs))
}

/// Minimize the given costs with at most one nonzero bounded continuous variable.
///
/// Original IDs are 0..len(costs). Selector for member i has ID len(costs) + i.
/// Exact zero-bound members remain original variables but need no selector.
/// With fewer than two remaining members, no rows, selectors, or hints are added.
#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
fn compile_sos1(py: Python<'_>, costs: Vec<f64>, bounds: Vec<(f64, f64)>) -> PyResult<PyInstance> {
    compile(py, || model::sos1(&costs, &bounds))
}

/// Build an absolute-value objective supported by Rust v3 but rejected by SDK 2.9.
#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
fn compile_absolute_objective(py: Python<'_>, costs: Vec<f64>) -> PyResult<PyInstance> {
    compile(py, || model::absolute_objective(&costs))
}

#[pymodule(gil_used = false)]
fn bridge_test_modeling(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(compile_one_hot, module)?)?;
    module.add_function(wrap_pyfunction!(compile_sos1, module)?)?;
    module.add_function(wrap_pyfunction!(compile_absolute_objective, module)?)?;
    Ok(())
}

pyo3_stub_gen::define_stub_info_gatherer!(stub_info);
