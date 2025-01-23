use anyhow::Result;
use pyo3::{
    prelude::*,
    types::{PyBytes, PyString},
};

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyfunction)]
#[pyfunction(name = "load_qplib_bytes")]
pub fn load_qplib_bytes<'py>(
    py: Python<'py>,
    path: Bound<PyString>,
) -> Result<Bound<'py, PyBytes>> {
    let instance = ommx::qplib::load_file_bytes(path.to_str()?)?;
    Ok(PyBytes::new_bound(py, &instance))
}
