use crate::message::Function;
use anyhow::Result;
use fnv::FnvHashMap;
use ommx::{ConstraintID, Equality, Message, Parse};
use pyo3::{
    prelude::*,
    types::PyBytes,
};
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
    #[pyo3(signature = (id, function, equality, name=None, subscripts=None))]
    pub fn new(
        id: u64,
        function: Function,
        equality: u32,
        name: Option<String>,
        subscripts: Option<Vec<i64>>,
    ) -> Result<Self> {
        let constraint_id = ConstraintID::from(id);
        let rust_equality = match equality {
            1 => Equality::EqualToZero,
            2 => Equality::LessThanOrEqualToZero,
            _ => return Err(anyhow::anyhow!("Invalid equality: {}", equality).into()),
        };

        let constraint = ommx::Constraint {
            id: constraint_id,
            function: function.0,
            equality: rust_equality,
            name,
            subscripts: subscripts.unwrap_or_default(),
            parameters: FnvHashMap::default(),
            description: None,
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

    #[staticmethod]
    #[pyo3(signature = (id, function, name=None))]
    pub fn equal_to_zero(id: u64, function: Function, name: Option<String>) -> Result<Self> {
        Self::new(
            id, function, 1, // EQUALITY_EQUAL_TO_ZERO
            name, None,
        )
    }

    #[staticmethod]
    #[pyo3(signature = (id, function, name=None))]
    pub fn less_than_or_equal_to_zero(
        id: u64,
        function: Function,
        name: Option<String>,
    ) -> Result<Self> {
        Self::new(
            id, function, 2, // EQUALITY_LESS_THAN_OR_EQUAL_TO_ZERO
            name, None,
        )
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

    pub fn __repr__(&self) -> String {
        format!(
            "Constraint(id={}, equality={}, name=\"{}\")",
            self.id(),
            match self.0.equality {
                ommx::Equality::EqualToZero => "EqualToZero",
                ommx::Equality::LessThanOrEqualToZero => "LessThanOrEqualToZero",
            },
            self.name()
        )
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

    pub fn __repr__(&self) -> String {
        format!(
            "RemovedConstraint(id={}, reason=\"{}\", name=\"{}\")",
            self.id(),
            self.removed_reason(),
            self.name()
        )
    }
}
