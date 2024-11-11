use anyhow::Result;
use ommx::{v1::Instance, Message};
use pyo3::{
    prelude::*,
    types::{PyBytes, PyString},
};

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyfunction)]
#[pyfunction(name = "load_mps_bytes")]
pub fn load_mps_bytes<'py>(py: Python<'py>, path: Bound<PyString>) -> Result<Bound<'py, PyBytes>> {
    let instance = ommx::mps::load_file_bytes(path.to_str()?)?;
    Ok(PyBytes::new_bound(py, &instance))
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyfunction)]
#[pyfunction(name = "write_mps_file")]
pub fn write_mps_file(instance: Bound<PyBytes>, path: Bound<PyString>) -> Result<()> {
    let instance = Instance::decode(instance.as_bytes())?;
    let path = path.to_str()?;
    ommx::mps::write_file(&instance, path)?;
    Ok(())
}
