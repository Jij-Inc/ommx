use anyhow::Result;
use pyo3::{prelude::*, types::PyBytes, Bound};

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct EvaluatedDecisionVariable(pub ommx::EvaluatedDecisionVariable);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl EvaluatedDecisionVariable {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        Ok(Self(ommx::EvaluatedDecisionVariable::from_bytes(bytes.as_bytes())?))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.0.to_bytes())
    }

    /// Get the variable ID
    #[getter]
    pub fn id(&self) -> u64 {
        self.0.id().into_inner()
    }

    /// Get the variable kind
    #[getter]
    pub fn kind(&self) -> crate::Kind {
        (*self.0.kind()).into()
    }

    /// Get the evaluated value
    #[getter]
    pub fn value(&self) -> f64 {
        *self.0.value()
    }

    /// Get the lower bound
    #[getter]
    pub fn lower_bound(&self) -> f64 {
        self.0.bound().lower()
    }

    /// Get the upper bound
    #[getter]
    pub fn upper_bound(&self) -> f64 {
        self.0.bound().upper()
    }

    /// Get the variable name
    #[getter]
    pub fn name(&self) -> Option<String> {
        self.0.metadata.name.clone()
    }

    /// Get the subscripts
    #[getter]
    pub fn subscripts(&self) -> Vec<i64> {
        self.0.metadata.subscripts.clone()
    }

    /// Get the parameters
    #[getter]
    pub fn parameters(&self) -> std::collections::HashMap<String, String> {
        self.0
            .metadata
            .parameters
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Get the description
    #[getter]
    pub fn description(&self) -> Option<String> {
        self.0.metadata.description.clone()
    }
}
