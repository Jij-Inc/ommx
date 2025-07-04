use std::collections::BTreeSet;

use anyhow::Result;
use pyo3::{prelude::*, types::PyBytes, Bound};

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct EvaluatedConstraint(pub ommx::EvaluatedConstraint);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl EvaluatedConstraint {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        Ok(Self(ommx::EvaluatedConstraint::from_bytes(bytes.as_bytes())?))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.0.to_bytes())
    }

    /// Get the constraint ID
    #[getter]
    pub fn id(&self) -> u64 {
        self.0.id().into_inner()
    }

    /// Get the constraint equality type
    #[getter]
    pub fn equality(&self) -> crate::Equality {
        (*self.0.equality()).into()
    }

    /// Get the evaluated constraint value
    #[getter]
    pub fn evaluated_value(&self) -> f64 {
        *self.0.evaluated_value()
    }

    /// Get the dual variable value
    #[getter]
    pub fn dual_variable(&self) -> Option<f64> {
        self.0.dual_variable
    }

    /// Set the dual variable value
    #[setter]
    pub fn set_dual_variable(&mut self, value: Option<f64>) {
        self.0.dual_variable = value;
    }

    /// Get the feasibility status
    #[getter]
    pub fn feasible(&self) -> bool {
        *self.0.feasible()
    }

    /// Get the removal reason
    #[getter]
    pub fn removed_reason(&self) -> Option<String> {
        self.0.removed_reason().clone()
    }

    /// Get the constraint name
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

    /// Get the used decision variable IDs
    #[getter]
    pub fn used_decision_variable_ids(&self) -> BTreeSet<u64> {
        self.0
            .used_decision_variable_ids()
            .iter()
            .map(|id| id.into_inner())
            .collect()
    }
}
