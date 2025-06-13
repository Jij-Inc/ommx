use ommx::{ConstraintID, VariableID};
use pyo3::{prelude::*, Bound, PyAny};
use std::collections::BTreeSet;

/// OneHot constraint hint wrapper for Python
#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
#[derive(Debug, Clone)]
pub struct OneHot(pub ommx::OneHot);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl OneHot {
    #[new]
    pub fn new(id: u64, variables: Vec<u64>) -> Self {
        let constraint_id = ConstraintID::from(id);
        let variable_set: BTreeSet<VariableID> = variables
            .into_iter()
            .map(VariableID::from)
            .collect();

        Self(ommx::OneHot {
            id: constraint_id,
            variables: variable_set,
        })
    }

    #[getter]
    pub fn id(&self) -> u64 {
        self.0.id.into_inner()
    }

    #[getter]
    pub fn variables(&self) -> Vec<u64> {
        self.0
            .variables
            .iter()
            .map(|v| v.into_inner())
            .collect()
    }

    pub fn __repr__(&self) -> String {
        format!(
            "OneHot(id={}, variables={:?})",
            self.id(),
            self.variables()
        )
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: Bound<'_, PyAny>) -> Self {
        self.clone()
    }
}

/// SOS1 constraint hint wrapper for Python
#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
#[derive(Debug, Clone)]
pub struct Sos1(pub ommx::Sos1);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl Sos1 {
    #[new]
    pub fn new(
        binary_constraint_id: u64,
        big_m_constraint_ids: Vec<u64>,
        variables: Vec<u64>,
    ) -> Self {
        let binary_constraint_id = ConstraintID::from(binary_constraint_id);
        let big_m_constraint_ids: BTreeSet<ConstraintID> = big_m_constraint_ids
            .into_iter()
            .map(ConstraintID::from)
            .collect();
        let variable_set: BTreeSet<VariableID> = variables
            .into_iter()
            .map(VariableID::from)
            .collect();

        Self(ommx::Sos1 {
            binary_constraint_id,
            big_m_constraint_ids,
            variables: variable_set,
        })
    }

    #[getter]
    pub fn binary_constraint_id(&self) -> u64 {
        self.0.binary_constraint_id.into_inner()
    }

    #[getter]
    pub fn big_m_constraint_ids(&self) -> Vec<u64> {
        self.0
            .big_m_constraint_ids
            .iter()
            .map(|c| c.into_inner())
            .collect()
    }

    #[getter]
    pub fn variables(&self) -> Vec<u64> {
        self.0
            .variables
            .iter()
            .map(|v| v.into_inner())
            .collect()
    }

    pub fn __repr__(&self) -> String {
        format!(
            "Sos1(binary_constraint_id={}, big_m_constraint_ids={:?}, variables={:?})",
            self.binary_constraint_id(),
            self.big_m_constraint_ids(),
            self.variables()
        )
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: Bound<'_, PyAny>) -> Self {
        self.clone()
    }
}

/// ConstraintHints wrapper for Python
#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
#[derive(Clone)]
pub struct ConstraintHints(pub ommx::ConstraintHints);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl ConstraintHints {
    #[new]
    #[pyo3(signature = (one_hot_constraints=Vec::new(), sos1_constraints=Vec::new()))]
    pub fn new(
        one_hot_constraints: Vec<OneHot>,
        sos1_constraints: Vec<Sos1>,
    ) -> Self {
        Self(ommx::ConstraintHints {
            one_hot_constraints: one_hot_constraints.into_iter().map(|oh| oh.0).collect(),
            sos1_constraints: sos1_constraints.into_iter().map(|s| s.0).collect(),
        })
    }

    #[getter]
    pub fn one_hot_constraints(&self) -> Vec<OneHot> {
        self.0
            .one_hot_constraints
            .iter()
            .cloned()
            .map(OneHot)
            .collect()
    }

    #[getter]
    pub fn sos1_constraints(&self) -> Vec<Sos1> {
        self.0
            .sos1_constraints
            .iter()
            .cloned()
            .map(Sos1)
            .collect()
    }

    pub fn __repr__(&self) -> String {
        format!(
            "ConstraintHints(one_hot_constraints={:?}, sos1_constraints={:?})",
            self.one_hot_constraints(),
            self.sos1_constraints()
        )
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: Bound<'_, PyAny>) -> Self {
        self.clone()
    }
}