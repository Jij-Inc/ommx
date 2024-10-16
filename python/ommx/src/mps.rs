use pyo3::{
    exceptions::PyValueError,
    prelude::*,
    types::{PyBytes, PyString},
};

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyfunction)]
#[pyfunction(name = "load_mps_bytes")]
pub fn load_mps_bytes<'py>(
    py: Python<'py>,
    path: Bound<PyString>,
) -> PyResult<Bound<'py, PyBytes>> {
    let instance = ommx::mps::load_file_bytes(path.to_str()?)
        .map_err(|err| PyValueError::new_err(format!("{}", err)))?;
    Ok(PyBytes::new_bound(py, &instance))
}
