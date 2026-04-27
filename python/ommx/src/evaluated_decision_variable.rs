use anyhow::Result;
use ommx::{evaluated_decision_variable_to_v1, v1, Message, Parse};
use pyo3::{prelude::*, types::PyBytes, Bound};

#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
pub struct EvaluatedDecisionVariable(
    pub ommx::EvaluatedDecisionVariable,
    pub ommx::DecisionVariableMetadata,
);

impl EvaluatedDecisionVariable {
    pub fn standalone(inner: ommx::EvaluatedDecisionVariable) -> Self {
        Self(inner, ommx::DecisionVariableMetadata::default())
    }

    pub fn from_parts(
        inner: ommx::EvaluatedDecisionVariable,
        metadata: ommx::DecisionVariableMetadata,
    ) -> Self {
        Self(inner, metadata)
    }
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl EvaluatedDecisionVariable {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        let inner = v1::DecisionVariable::decode(bytes.as_bytes())?;
        let parsed: ommx::decision_variable::parse::ParsedDecisionVariable =
            Parse::parse(inner, &())?;
        let metadata = parsed.metadata;
        let parsed_dv = parsed.variable;
        let value = parsed_dv
            .substituted_value()
            .ok_or_else(|| anyhow::anyhow!("Missing value for EvaluatedDecisionVariable"))?;
        let evaluated =
            ommx::EvaluatedDecisionVariable::new(parsed_dv, value, ommx::ATol::default())?;
        Ok(Self(evaluated, metadata))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        let v1_dv = evaluated_decision_variable_to_v1(self.0.clone(), self.1.clone());
        PyBytes::new(py, &v1_dv.encode_to_vec())
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
        self.1.name.clone()
    }

    /// Get the subscripts
    #[getter]
    pub fn subscripts(&self) -> Vec<i64> {
        self.1.subscripts.clone()
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

    /// Get the description
    #[getter]
    pub fn description(&self) -> Option<String> {
        self.1.description.clone()
    }
}
