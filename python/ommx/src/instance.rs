use anyhow::Result;
use ommx::Message;
use pyo3::{
    prelude::*,
    types::{PyBytes, PyDict},
};

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct Instance(ommx::v1::Instance);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl Instance {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        let inner = ommx::v1::Instance::decode(bytes.as_bytes())?;
        Ok(Self(inner))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        Ok(PyBytes::new_bound(py, &self.0.encode_to_vec()))
    }

    pub fn to_pubo<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyDict>> {
        let pubo = self.0.to_pubo()?;
        Ok(serde_pyobject::to_pyobject(py, &pubo)?.extract()?)
    }

    pub fn to_qubo<'py>(&self, py: Python<'py>) -> Result<(Bound<'py, PyDict>, f64)> {
        let (qubo, constant) = self.0.to_qubo()?;
        Ok((serde_pyobject::to_pyobject(py, &qubo)?.extract()?, constant))
    }
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct ParametricInstance(ommx::v1::ParametricInstance);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl ParametricInstance {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        let inner = ommx::v1::ParametricInstance::decode(bytes.as_bytes())?;
        Ok(Self(inner))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        Ok(PyBytes::new_bound(py, &self.0.encode_to_vec()))
    }
}
