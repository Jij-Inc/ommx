use pyo3::prelude::*;
use std::collections::BTreeMap;

#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
pub struct SampledDecisionVariable(
    pub ommx::VariableID,
    pub ommx::SampledDecisionVariable,
    pub ommx::DecisionVariableLabel,
);

impl SampledDecisionVariable {
    pub fn from_parts(
        id: ommx::VariableID,
        inner: ommx::SampledDecisionVariable,
        label: ommx::DecisionVariableLabel,
    ) -> Self {
        Self(id, inner, label)
    }
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl SampledDecisionVariable {
    /// Get the decision variable ID
    #[getter]
    pub fn id(&self) -> u64 {
        self.0.into_inner()
    }

    /// Get the decision variable kind
    #[getter]
    pub fn kind(&self) -> crate::Kind {
        (*self.1.kind()).into()
    }

    /// Get the decision variable bound
    #[getter]
    pub fn bound(&self) -> crate::VariableBound {
        crate::VariableBound(*self.1.bound())
    }

    /// Explicit feasible values for a finite-domain variable.
    #[getter]
    pub fn values(&self) -> Option<Vec<f64>> {
        self.1
            .finite_domain()
            .map(|domain| domain.values().to_vec())
    }

    /// Get the decision variable name
    #[getter]
    pub fn name(&self) -> Option<String> {
        self.2.name.clone()
    }

    /// Get the subscripts
    #[getter]
    pub fn subscripts(&self) -> Vec<i64> {
        self.2.subscripts.clone()
    }

    /// Get the description
    #[getter]
    pub fn description(&self) -> Option<String> {
        self.2.description.clone()
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

    /// Get the sampled values for all samples
    #[getter]
    pub fn samples(&self) -> BTreeMap<u64, f64> {
        self.1
            .samples()
            .iter()
            .map(|(sample_id, value)| (sample_id.into_inner(), *value))
            .collect()
    }
}
