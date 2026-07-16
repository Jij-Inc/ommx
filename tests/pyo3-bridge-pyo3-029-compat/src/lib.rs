use ommx_pyo3_bridge::PyFunction;
use pyo3::prelude::*;

#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
fn function() -> PyFunction {
    ommx::Function::default().into()
}

#[pymodule(gil_used = false)]
fn ommx_pyo3_bridge_pyo3_029_compat(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(function, module)?)?;
    Ok(())
}
