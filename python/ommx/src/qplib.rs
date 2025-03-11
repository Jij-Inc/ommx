use anyhow::Result;
use pyo3::{prelude::*, types::PyBytes};

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyfunction)]
#[pyfunction(name = "load_qplib_bytes")]
pub fn load_qplib_bytes<'py>(py: Python<'py>, path: String) -> Result<Bound<'py, PyBytes>> {
    let instance = ommx::qplib::load_file_bytes(path)?;
    Ok(PyBytes::new(py, &instance))
}
