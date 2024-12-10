use anyhow::Result;
use ommx::Message;
use pyo3::{
    prelude::*,
    types::{PyBytes, PyDict},
};

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyfunction)]
#[pyfunction]
pub fn instance_to_pubo<'py>(
    py: Python<'py>,
    instance_bytes: Bound<'_, PyBytes>,
) -> Result<Bound<'py, PyDict>> {
    let instance = ommx::v1::Instance::decode(instance_bytes.as_bytes())?;
    let pubo = instance.to_pubo()?;
    Ok(serde_pyobject::to_pyobject(py, &pubo)?.extract()?)
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyfunction)]
#[pyfunction]
pub fn instance_to_qubo<'py>(
    py: Python<'py>,
    instance_bytes: Bound<'_, PyBytes>,
) -> Result<(Bound<'py, PyDict>, f64)> {
    let instance = ommx::v1::Instance::decode(instance_bytes.as_bytes())?;
    let (qubo, constant) = instance.to_qubo()?;
    Ok((serde_pyobject::to_pyobject(py, &qubo)?.extract()?, constant))
}

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
