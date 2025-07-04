use crate::VariableBound;
use anyhow::Result;
use ommx::{v1, ATol, VariableID};
use pyo3::{prelude::*, types::PyBytes, Bound, PyAny};
use std::collections::HashMap;

/// DecisionVariable wrapper for Python
#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
#[derive(Clone)]
pub struct DecisionVariable(pub ommx::DecisionVariable);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl DecisionVariable {
    #[new]
    #[pyo3(signature = (id, kind, bound, name=None, subscripts=Vec::new(), parameters=HashMap::default(), description=None))]
    pub fn new(
        id: u64,
        kind: i32,
        bound: VariableBound,
        name: Option<String>,
        subscripts: Vec<i64>,
        parameters: HashMap<String, String>,
        description: Option<String>,
    ) -> Result<Self> {
        let variable_id = VariableID::from(id);
        let kind = v1::decision_variable::Kind::try_from(kind)?.try_into()?;

        let mut decision_variable = ommx::DecisionVariable::new(
            variable_id,
            kind,
            bound.0,
            None, // substituted_value
            ATol::default(),
        )?;

        decision_variable.metadata.name = name;
        decision_variable.metadata.subscripts = subscripts;
        decision_variable.metadata.parameters = parameters.into_iter().collect();
        decision_variable.metadata.description = description;

        Ok(Self(decision_variable))
    }

    #[getter]
    pub fn id(&self) -> u64 {
        self.0.id().into_inner()
    }

    #[getter]
    pub fn kind(&self) -> i32 {
        let kind: v1::decision_variable::Kind = self.0.kind().into();
        kind as i32
    }

    #[getter]
    pub fn bound(&self) -> VariableBound {
        VariableBound(self.0.bound())
    }

    #[getter]
    pub fn name(&self) -> String {
        self.0.metadata.name.clone().unwrap_or_default()
    }

    #[getter]
    pub fn subscripts(&self) -> Vec<i64> {
        self.0.metadata.subscripts.clone()
    }

    #[getter]
    pub fn parameters(&self) -> HashMap<String, String> {
        self.0
            .metadata
            .parameters
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    #[getter]
    pub fn description(&self) -> String {
        self.0.metadata.description.clone().unwrap_or_default()
    }

    #[getter]
    pub fn substituted_value(&self) -> Option<f64> {
        self.0.substituted_value()
    }

    #[staticmethod]
    #[pyo3(signature = (id, name=None, subscripts=Vec::new(), parameters=HashMap::default(), description=None))]
    pub fn binary(
        id: u64,
        name: Option<String>,
        subscripts: Vec<i64>,
        parameters: HashMap<String, String>,
        description: Option<String>,
    ) -> Result<Self> {
        Self::new(
            id,
            1, // KIND_BINARY
            VariableBound(ommx::Bound::of_binary()),
            name,
            subscripts,
            parameters,
            description,
        )
    }

    #[staticmethod]
    #[pyo3(signature = (id, bound, name=None, subscripts=Vec::new(), parameters=HashMap::default(), description=None))]
    pub fn integer(
        id: u64,
        bound: VariableBound,
        name: Option<String>,
        subscripts: Vec<i64>,
        parameters: HashMap<String, String>,
        description: Option<String>,
    ) -> Result<Self> {
        Self::new(
            id,
            2, // KIND_INTEGER
            bound,
            name,
            subscripts,
            parameters,
            description,
        )
    }

    #[staticmethod]
    #[pyo3(signature = (id, bound, name=None, subscripts=Vec::new(), parameters=HashMap::default(), description=None))]
    pub fn continuous(
        id: u64,
        bound: VariableBound,
        name: Option<String>,
        subscripts: Vec<i64>,
        parameters: HashMap<String, String>,
        description: Option<String>,
    ) -> Result<Self> {
        Self::new(
            id,
            3, // KIND_CONTINUOUS
            bound,
            name,
            subscripts,
            parameters,
            description,
        )
    }

    #[staticmethod]
    #[pyo3(signature = (id, bound, name=None, subscripts=Vec::new(), parameters=HashMap::default(), description=None))]
    pub fn semi_integer(
        id: u64,
        bound: VariableBound,
        name: Option<String>,
        subscripts: Vec<i64>,
        parameters: HashMap<String, String>,
        description: Option<String>,
    ) -> Result<Self> {
        Self::new(
            id,
            4, // KIND_SEMI_INTEGER
            bound,
            name,
            subscripts,
            parameters,
            description,
        )
    }

    #[staticmethod]
    #[pyo3(signature = (id, bound, name=None, subscripts=Vec::new(), parameters=HashMap::default(), description=None))]
    pub fn semi_continuous(
        id: u64,
        bound: VariableBound,
        name: Option<String>,
        subscripts: Vec<i64>,
        parameters: HashMap<String, String>,
        description: Option<String>,
    ) -> Result<Self> {
        Self::new(
            id,
            5, // KIND_SEMI_CONTINUOUS
            bound,
            name,
            subscripts,
            parameters,
            description,
        )
    }

    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        Ok(Self(ommx::DecisionVariable::from_bytes(bytes.as_bytes())?))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.0.to_bytes())
    }

    pub fn __repr__(&self) -> String {
        format!(
            "DecisionVariable(id={}, kind={}, name=\"{}\", bound=[{}, {}])",
            self.id(),
            self.kind(),
            self.name(),
            self.0.bound().lower(),
            self.0.bound().upper()
        )
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    // __deepcopy__ can also be implemented with self.clone()
    // memo argument is required to match Python protocol but not used in this implementation
    // Since this implementation contains no PyObject references, simple clone is sufficient
    fn __deepcopy__(&self, _memo: Bound<'_, PyAny>) -> Self {
        self.clone()
    }
}
