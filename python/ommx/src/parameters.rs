use crate::error::OmmxPyResult;
use ommx::Message;
use pyo3::{prelude::*, types::PyBytes, Bound};

#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
pub struct Parameters(pub ommx::v1::Parameters);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl Parameters {
    #[staticmethod]
    pub fn from_v1_bytes(bytes: &Bound<PyBytes>) -> OmmxPyResult<Self> {
        let inner = crate::message_io::decode(bytes.as_bytes(), "ommx.v1.Parameters")?;
        Ok(Self(inner))
    }

    pub fn to_v1_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.0.encode_to_vec())
    }
}
