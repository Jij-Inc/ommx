use anyhow::Result;
use ommx::{Message, Parse};
use pyo3::{prelude::*, types::PyBytes, Bound};
use std::collections::BTreeMap;

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct SampledDecisionVariable(pub ommx::SampledDecisionVariable);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl SampledDecisionVariable {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        let v1_variable = ommx::v1::SampledDecisionVariable::decode(bytes.as_bytes())?;
        let variable = v1_variable.parse(&())?;
        Ok(Self(variable))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let v1_variable: ommx::v1::SampledDecisionVariable = self.0.clone().into();
        Ok(PyBytes::new(py, &v1_variable.encode_to_vec()))
    }

    /// Get the decision variable
    #[getter]
    pub fn decision_variable(&self) -> crate::DecisionVariable {
        // Create a minimal DecisionVariable from metadata
        let metadata = &self.0.metadata;
        let dv = ommx::DecisionVariable::binary(ommx::VariableID::from(metadata.id));
        crate::DecisionVariable(dv)
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