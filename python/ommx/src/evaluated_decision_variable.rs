use pyo3::prelude::*;

#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
pub struct EvaluatedDecisionVariable(
    pub ommx::VariableID,
    pub ommx::EvaluatedDecisionVariable,
    pub ommx::DecisionVariableLabel,
);

impl EvaluatedDecisionVariable {
    pub fn from_parts(
        id: ommx::VariableID,
        inner: ommx::EvaluatedDecisionVariable,
        label: ommx::DecisionVariableLabel,
    ) -> Self {
        Self(id, inner, label)
    }
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl EvaluatedDecisionVariable {
    /// Get the variable ID
    #[getter]
    pub fn id(&self) -> u64 {
        self.0.into_inner()
    }

    /// Get the variable kind
    #[getter]
    pub fn kind(&self) -> crate::Kind {
        (*self.1.kind()).into()
    }

    /// Get the evaluated value
    #[getter]
    pub fn value(&self) -> f64 {
        *self.1.value()
    }

    /// Get the lower bound
    #[getter]
    pub fn lower_bound(&self) -> f64 {
        self.1.bound().lower()
    }

    /// Get the upper bound
    #[getter]
    pub fn upper_bound(&self) -> f64 {
        self.1.bound().upper()
    }

    /// Explicit feasible values for a finite-domain variable.
    #[getter]
    pub fn values(&self) -> Option<Vec<f64>> {
        self.1
            .finite_domain()
            .map(|domain| domain.values().to_vec())
    }

    /// Get the variable name
    #[getter]
    pub fn name(&self) -> Option<String> {
        self.2.name.clone()
    }

    /// Get the subscripts
    #[getter]
    pub fn subscripts(&self) -> Vec<i64> {
        self.2.subscripts.clone()
    }

    /// Get the parameters
    #[getter]
    pub fn parameters(&self) -> std::collections::HashMap<String, String> {
        self.2
            .parameters
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Get the description
    #[getter]
    pub fn description(&self) -> Option<String> {
        self.2.description.clone()
    }
}
