use anyhow::Result;
use ommx::Message;
use pyo3::{prelude::*, types::PyBytes, Bound};

#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
pub struct Parameters(pub ommx::v1::Parameters);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl Parameters {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        let inner = ommx::v1::Parameters::decode(bytes.as_bytes())?;
        Ok(Self(inner))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.0.encode_to_vec())
    }
}
