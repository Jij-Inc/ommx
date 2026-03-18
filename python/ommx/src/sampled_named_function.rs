use anyhow::Result;
use pyo3::{prelude::*, types::PyBytes, Bound};
use std::collections::{BTreeMap, HashSet};

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
#[derive(Clone)]
pub struct SampledNamedFunction(pub ommx::SampledNamedFunction);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl SampledNamedFunction {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        Ok(Self(ommx::SampledNamedFunction::from_bytes(
            bytes.as_bytes(),
        )?))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.0.to_bytes())
    }

    /// Get the named function ID
    #[getter]
    pub fn id(&self) -> u64 {
        self.0.id().into_inner()
    }

    /// Get the named function name
    #[getter]
    pub fn name(&self) -> Option<String> {
        self.0.name.clone()
    }

    /// Get the subscripts
    #[getter]
    pub fn subscripts(&self) -> Vec<i64> {
        self.0.subscripts.clone()
    }

    /// Get the description
    #[getter]
    pub fn description(&self) -> Option<String> {
        self.0.description.clone()
    }

    /// Get the parameters
    #[getter]
    pub fn parameters(&self) -> std::collections::HashMap<String, String> {
        self.0
            .parameters
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Get the sampled values for all samples
    #[getter]
    pub fn evaluated_values(&self) -> BTreeMap<u64, f64> {
        self.0
            .evaluated_values()
            .iter()
            .map(|(sample_id, value)| (sample_id.into_inner(), *value))
            .collect()
    }

    #[getter]
    pub fn used_decision_variable_ids(&self) -> HashSet<u64> {
        self.0
            .used_decision_variable_ids()
            .iter()
            .map(|id| id.into_inner())
            .collect()
    }

    pub fn __repr__(&self) -> String {
        let name_str = self
            .0
            .name
            .as_ref()
            .map(|n| format!("\"{n}\""))
            .unwrap_or_else(|| "None".to_string());
        let num_samples = self.0.evaluated_values().num_samples();
        format!(
            "SampledNamedFunction(id={}, name={}, subscripts={:?}, num_samples={})",
            self.0.id(),
            name_str,
            self.0.subscripts,
            num_samples
        )
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: Bound<'_, PyAny>) -> Self {
        self.clone()
    }
}
