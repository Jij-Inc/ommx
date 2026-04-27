use pyo3::prelude::*;
use std::collections::BTreeMap;

#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
pub struct SampledDecisionVariable(
    pub ommx::SampledDecisionVariable,
    pub ommx::DecisionVariableMetadata,
);

impl SampledDecisionVariable {
    pub fn standalone(inner: ommx::SampledDecisionVariable) -> Self {
        Self(inner, ommx::DecisionVariableMetadata::default())
    }

    pub fn from_parts(
        inner: ommx::SampledDecisionVariable,
        metadata: ommx::DecisionVariableMetadata,
    ) -> Self {
        Self(inner, metadata)
    }
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl SampledDecisionVariable {
    /// Get the decision variable ID
    #[getter]
    pub fn id(&self) -> u64 {
        self.0.id().into_inner()
    }

    /// Get the decision variable kind
    #[getter]
    pub fn kind(&self) -> crate::Kind {
        (*self.0.kind()).into()
    }

    /// Get the decision variable bound
    #[getter]
    pub fn bound(&self) -> crate::VariableBound {
        crate::VariableBound(*self.0.bound())
    }

    /// Get the decision variable name
    #[getter]
    pub fn name(&self) -> Option<String> {
        self.1.name.clone()
    }

    /// Get the subscripts
    #[getter]
    pub fn subscripts(&self) -> Vec<i64> {
        self.1.subscripts.clone()
    }

    /// Get the description
    #[getter]
    pub fn description(&self) -> Option<String> {
        self.1.description.clone()
    }

    /// Get the parameters
    #[getter]
    pub fn parameters(&self) -> std::collections::HashMap<String, String> {
        self.1
            .parameters
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Get the sampled values for all samples
    #[getter]
    pub fn samples(&self) -> BTreeMap<u64, f64> {
        self.0
            .samples()
            .iter()
            .map(|(sample_id, value)| (sample_id.into_inner(), *value))
            .collect()
    }
}
