use anyhow::Result;
use ommx::Message;
use pyo3::{prelude::*, types::PyBytes, Bound};

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct SampledValues(pub ommx::v1::SampledValues);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl SampledValues {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        let values = ommx::v1::SampledValues::decode(bytes.as_bytes())?;
        Ok(Self(values))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        Ok(PyBytes::new(py, &self.0.encode_to_vec()))
    }

    /// Get entries for iteration
    #[getter]
    pub fn entries(&self) -> Vec<SampledValuesEntry> {
        self.0
            .entries
            .iter()
            .map(|entry| SampledValuesEntry(entry.clone()))
            .collect()
    }
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct SampledValuesEntry(pub ommx::v1::sampled_values::SampledValuesEntry);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl SampledValuesEntry {
    /// Get the sample IDs
    #[getter]
    pub fn ids(&self) -> Vec<u64> {
        self.0.ids.clone()
    }

    /// Get the value
    #[getter]
    pub fn value(&self) -> f64 {
        self.0.value
    }
}