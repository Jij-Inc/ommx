use anyhow::Result;
use pyo3::{prelude::*, types::PyBytes, Bound};
use std::collections::{HashMap, HashSet};

/// EvaluatedNamedFunction wrapper for Python
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct EvaluatedNamedFunction(pub ommx::EvaluatedNamedFunction);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl EvaluatedNamedFunction {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        Ok(Self(ommx::EvaluatedNamedFunction::from_bytes(
            bytes.as_bytes(),
        )?))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.0.to_bytes())
    }

    #[getter]
    pub fn id(&self) -> u64 {
        self.0.id.into_inner()
    }

    #[getter]
    pub fn evaluated_value(&self) -> f64 {
        self.0.evaluated_value()
    }

    #[getter]
    pub fn name(&self) -> Option<String> {
        self.0.name.clone()
    }

    #[getter]
    pub fn subscripts(&self) -> Vec<i64> {
        self.0.subscripts.clone()
    }

    #[getter]
    pub fn parameters(&self) -> HashMap<String, String> {
        self.0.parameters.clone().into_iter().collect()
    }

    #[getter]
    pub fn description(&self) -> Option<String> {
        self.0.description.clone()
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
        self.0.to_string()
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: Bound<'_, PyAny>) -> Self {
        self.clone()
    }
}
