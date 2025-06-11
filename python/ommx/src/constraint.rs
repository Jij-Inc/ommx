use crate::message::Function;
use anyhow::{anyhow, Result};
use fnv::FnvHashMap;
use ommx::{ConstraintID, Equality, Message, Parse};
use pyo3::{prelude::*, types::PyBytes};
use std::collections::HashMap;

/// Constraint wrapper for Python
#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
#[derive(Clone)]
pub struct Constraint(pub ommx::Constraint);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl Constraint {
    #[new]
    #[pyo3(signature = (id, function, equality, name=None, subscripts=Vec::new(), description=None, parameters=HashMap::default()))]
    pub fn new(
        id: u64,
        function: Function,
        equality: u32,
        name: Option<String>,
        subscripts: Vec<i64>,
        description: Option<String>,
        parameters: HashMap<String, String>,
    ) -> Result<Self> {
        let constraint_id = ConstraintID::from(id);
        let rust_equality = match equality {
            1 => Equality::EqualToZero,
            2 => Equality::LessThanOrEqualToZero,
            _ => return Err(anyhow!("Invalid equality: {}", equality)),
        };

        let constraint = ommx::Constraint {
            id: constraint_id,
            function: function.0,
            equality: rust_equality,
            name,
            subscripts,
            parameters: parameters.into_iter().collect(),
            description,
        };

        Ok(Self(constraint))
    }

    #[getter]
    pub fn id(&self) -> u64 {
        self.0.id.into_inner()
    }

    #[getter]
    pub fn function(&self) -> Function {
        Function(self.0.function.clone())
    }

    #[getter]
    pub fn equality(&self) -> u32 {
        match self.0.equality {
            ommx::Equality::EqualToZero => 1,
            ommx::Equality::LessThanOrEqualToZero => 2,
        }
    }

    #[getter]
    pub fn name(&self) -> String {
        self.0.name.clone().unwrap_or_default()
    }

    #[getter]
    pub fn subscripts(&self) -> Vec<i64> {
        self.0.subscripts.clone()
    }

    #[getter]
    pub fn description(&self) -> String {
        self.0.description.clone().unwrap_or_default()
    }

    #[getter]
    pub fn parameters(&self) -> HashMap<String, String> {
        self.0
            .parameters
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    #[staticmethod]
    pub fn decode(bytes: &Bound<PyBytes>) -> Result<Self> {
        let inner = ommx::v1::Constraint::decode(bytes.as_bytes())?;
        let parsed = Parse::parse(inner, &())?;
        Ok(Self(parsed))
    }

    pub fn encode<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let inner: ommx::v1::Constraint = self.0.clone().into();
        Ok(PyBytes::new(py, &inner.encode_to_vec()))
    }

    /// Set the name of the constraint
    pub fn set_name(&mut self, name: String) {
        self.0.name = Some(name);
    }

    /// Set the subscripts of the constraint
    pub fn set_subscripts(&mut self, subscripts: Vec<i64>) {
        self.0.subscripts = subscripts;
    }

    /// Add subscripts to the constraint
    pub fn add_subscripts(&mut self, subscripts: Vec<i64>) {
        self.0.subscripts.extend(subscripts);
    }

    /// Set the ID of the constraint
    pub fn set_id(&mut self, id: u64) {
        self.0.id = ConstraintID::from(id);
    }

    /// Set the description of the constraint
    pub fn set_description(&mut self, description: String) {
        self.0.description = Some(description);
    }

    /// Set the parameters of the constraint
    pub fn set_parameters(&mut self, parameters: HashMap<String, String>) {
        self.0.parameters = parameters.into_iter().collect();
    }

    /// Add a parameter to the constraint
    pub fn add_parameter(&mut self, key: String, value: String) {
        self.0.parameters.insert(key, value);
    }

    pub fn __repr__(&self) -> String {
        self.0.to_string()
    }
}

/// RemovedConstraint wrapper for Python
#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
#[derive(Clone)]
pub struct RemovedConstraint(pub ommx::RemovedConstraint);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl RemovedConstraint {
    #[new]
    #[pyo3(signature = (constraint, removed_reason, removed_reason_parameters=None))]
    pub fn new(
        constraint: Constraint,
        removed_reason: String,
        removed_reason_parameters: Option<HashMap<String, String>>,
    ) -> Self {
        let removed_constraint = ommx::RemovedConstraint {
            constraint: constraint.0,
            removed_reason,
            removed_reason_parameters: removed_reason_parameters
                .map(|params| params.into_iter().collect::<FnvHashMap<_, _>>())
                .unwrap_or_default(),
        };

        Self(removed_constraint)
    }

    #[getter]
    pub fn constraint(&self) -> Constraint {
        Constraint(self.0.constraint.clone())
    }

    #[getter]
    pub fn removed_reason(&self) -> String {
        self.0.removed_reason.clone()
    }

    #[getter]
    pub fn removed_reason_parameters(&self) -> HashMap<String, String> {
        self.0
            .removed_reason_parameters
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    #[getter]
    pub fn id(&self) -> u64 {
        self.0.constraint.id.into_inner()
    }

    #[getter]
    pub fn name(&self) -> String {
        self.0.constraint.name.clone().unwrap_or_default()
    }

    #[staticmethod]
    pub fn decode(bytes: &Bound<PyBytes>) -> Result<Self> {
        let inner = ommx::v1::RemovedConstraint::decode(bytes.as_bytes())?;
        let parsed = Parse::parse(inner, &())?;
        Ok(Self(parsed))
    }

    pub fn encode<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let inner: ommx::v1::RemovedConstraint = self.0.clone().into();
        Ok(PyBytes::new(py, &inner.encode_to_vec()))
    }

    pub fn __repr__(&self) -> String {
        self.0.to_string()
    }
}
