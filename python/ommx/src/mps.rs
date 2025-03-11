use anyhow::Result;
use ommx::{v1::Instance, Message};
use pyo3::{prelude::*, types::PyBytes};

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyfunction)]
#[pyfunction(name = "load_mps_bytes")]
pub fn load_mps_bytes<'py>(py: Python<'py>, path: String) -> Result<Bound<'py, PyBytes>> {
    let instance = ommx::mps::load_file_bytes(path)?;
    Ok(PyBytes::new(py, &instance))
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyfunction)]
#[pyfunction(name = "write_mps_file")]
pub fn write_mps_file(instance: Bound<PyBytes>, path: String) -> Result<()> {
    let instance = Instance::decode(instance.as_bytes())?;
    ommx::mps::write_file(&instance, path)?;
    Ok(())
}
