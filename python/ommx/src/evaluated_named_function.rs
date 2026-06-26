use pyo3::{prelude::*, Bound};
use std::collections::{HashMap, HashSet};

/// EvaluatedNamedFunction wrapper for Python.
///
/// Holds the Rust `EvaluatedNamedFunction` plus an owned snapshot of its
/// label. See `NamedFunction` for the snapshot-model rationale.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct EvaluatedNamedFunction(
    pub ommx::NamedFunctionID,
    pub ommx::EvaluatedNamedFunction,
    pub ommx::NamedFunctionLabel,
);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl EvaluatedNamedFunction {
    #[getter]
    pub fn id(&self) -> u64 {
        self.0.into_inner()
    }

    #[getter]
    pub fn evaluated_value(&self) -> f64 {
        self.1.evaluated_value()
    }

    #[getter]
    pub fn name(&self) -> Option<String> {
        self.2.name.clone()
    }

    #[getter]
    pub fn subscripts(&self) -> Vec<i64> {
        self.2.subscripts.clone()
    }

    #[getter]
    pub fn parameters(&self) -> HashMap<String, String> {
        self.2.parameters.clone().into_iter().collect()
    }

    #[getter]
    pub fn description(&self) -> Option<String> {
        self.2.description.clone()
    }

    #[getter]
    pub fn used_decision_variable_ids(&self) -> HashSet<u64> {
        self.1
            .used_decision_variable_ids()
            .iter()
            .map(|id| id.into_inner())
            .collect()
    }

    pub fn __repr__(&self) -> String {
        format!(
            "EvaluatedNamedFunction(id={}, value={})",
            self.0.into_inner(),
            self.1.evaluated_value()
        )
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: Bound<'_, PyAny>) -> Self {
        self.clone()
    }
}
