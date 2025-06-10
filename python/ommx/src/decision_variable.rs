use crate::message::VariableBound;
use anyhow::Result;
use fnv::FnvHashMap;
use ommx::{ATol, Kind, Message, Parse, VariableID};
use pyo3::{prelude::*, types::PyBytes};

/// DecisionVariable wrapper for Python
#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
#[derive(Clone)]
pub struct DecisionVariable(pub ommx::DecisionVariable);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl DecisionVariable {
    #[new]
    #[pyo3(signature = (id, kind, bound, name=None, subscripts=None))]
    pub fn new(
        id: u64,
        kind: u32,
        bound: VariableBound,
        name: Option<String>,
        subscripts: Option<Vec<i64>>,
    ) -> Result<Self> {
        let variable_id = VariableID::from(id);
        let rust_kind = match kind {
            1 => Kind::Binary,
            2 => Kind::Integer,
            3 => Kind::Continuous,
            4 => Kind::SemiInteger,
            5 => Kind::SemiContinuous,
            _ => return Err(anyhow::anyhow!("Invalid kind: {}", kind).into()),
        };

        let mut decision_variable = ommx::DecisionVariable::new(
            variable_id,
            rust_kind,
            bound.0,
            None, // substituted_value
            ATol::default(),
        )?;

        decision_variable.name = name;
        decision_variable.subscripts = subscripts.unwrap_or_default();
        decision_variable.parameters = FnvHashMap::default();
        decision_variable.description = None;

        Ok(Self(decision_variable))
    }

    #[getter]
    pub fn id(&self) -> u64 {
        self.0.id().into_inner()
    }

    #[getter]
    pub fn kind(&self) -> u32 {
        match self.0.kind() {
            ommx::Kind::Binary => 1,
            ommx::Kind::Integer => 2,
            ommx::Kind::Continuous => 3,
            ommx::Kind::SemiInteger => 4,
            ommx::Kind::SemiContinuous => 5,
        }
    }

    #[getter]
    pub fn bound(&self) -> VariableBound {
        VariableBound(self.0.bound())
    }

    #[getter]
    pub fn name(&self) -> String {
        self.0.name.clone().unwrap_or_default()
    }

    #[getter]
    pub fn subscripts(&self) -> Vec<i64> {
        self.0.subscripts.clone()
    }

    #[staticmethod]
    #[pyo3(signature = (id, name=None))]
    pub fn binary(id: u64, name: Option<String>) -> Result<Self> {
        Self::new(
            id,
            1, // KIND_BINARY
            VariableBound(ommx::Bound::of_binary()),
            name,
            None,
        )
    }

    #[staticmethod]
    #[pyo3(signature = (id, lower=None, upper=None, name=None))]
    pub fn integer(
        id: u64,
        lower: Option<f64>,
        upper: Option<f64>,
        name: Option<String>,
    ) -> Result<Self> {
        let bound = VariableBound(ommx::Bound::new(
            lower.unwrap_or(f64::NEG_INFINITY),
            upper.unwrap_or(f64::INFINITY),
        )?);
        Self::new(
            id, 2, // KIND_INTEGER
            bound, name, None,
        )
    }

    #[staticmethod]
    #[pyo3(signature = (id, lower=None, upper=None, name=None))]
    pub fn continuous(
        id: u64,
        lower: Option<f64>,
        upper: Option<f64>,
        name: Option<String>,
    ) -> Result<Self> {
        let bound = VariableBound(ommx::Bound::new(
            lower.unwrap_or(f64::NEG_INFINITY),
            upper.unwrap_or(f64::INFINITY),
        )?);
        Self::new(
            id, 3, // KIND_CONTINUOUS
            bound, name, None,
        )
    }

    #[staticmethod]
    pub fn decode(bytes: &Bound<PyBytes>) -> Result<Self> {
        let inner = ommx::v1::DecisionVariable::decode(bytes.as_bytes())?;
        let parsed = Parse::parse(inner, &())?;
        Ok(Self(parsed))
    }

    pub fn encode<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let inner: ommx::v1::DecisionVariable = self.0.clone().into();
        Ok(PyBytes::new(py, &inner.encode_to_vec()))
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
}
