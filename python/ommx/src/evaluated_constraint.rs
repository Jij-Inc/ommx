use std::collections::BTreeSet;

use anyhow::Result;
use pyo3::{prelude::*, types::PyBytes, Bound};

#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
pub struct EvaluatedConstraint(pub ommx::EvaluatedConstraint);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl EvaluatedConstraint {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        let (_id, constraint) = ommx::EvaluatedConstraint::from_bytes(bytes.as_bytes())?;
        Ok(Self(constraint))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.0.to_bytes(ommx::ConstraintID::from(0)))
    }

    /// Get the constraint equality type
    #[getter]
    pub fn equality(&self) -> crate::Equality {
        self.0.equality.into()
    }

    /// Get the evaluated constraint value
    #[getter]
    pub fn evaluated_value(&self) -> f64 {
        self.0.stage.evaluated_value
    }

    /// Get the dual variable value
    #[getter]
    pub fn dual_variable(&self) -> Option<f64> {
        self.0.stage.dual_variable
    }

    /// Set the dual variable value
    #[setter]
    pub fn set_dual_variable(&mut self, value: Option<f64>) {
        self.0.stage.dual_variable = value;
    }

    /// Get the feasibility status
    #[getter]
    pub fn feasible(&self) -> bool {
        self.0.stage.feasible
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
            .stage
            .used_decision_variable_ids
            .iter()
            .map(|id| id.into_inner())
            .collect()
    }

    /// Calculate the violation (constraint breach) value for this constraint
    ///
    /// Returns the amount by which this constraint is violated:
    /// - For $f(x) = 0$: returns $|f(x)|$
    /// - For $f(x) \leq 0$: returns $\max(0, f(x))$
    ///
    /// Returns 0.0 if the constraint is satisfied.
    pub fn violation(&self) -> f64 {
        self.0.violation()
    }
}
