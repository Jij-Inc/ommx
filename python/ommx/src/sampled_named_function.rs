use pyo3::{prelude::*, Bound};
use std::collections::{BTreeMap, HashSet};

/// SampledNamedFunction wrapper for Python.
///
/// Holds the Rust `SampledNamedFunction` plus an owned snapshot of its
/// label. See `NamedFunction` for the snapshot-model rationale.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct SampledNamedFunction(
    pub ommx::NamedFunctionID,
    pub ommx::SampledNamedFunction,
    pub ommx::NamedFunctionLabel,
);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl SampledNamedFunction {
    /// Get the named function ID
    #[getter]
    pub fn id(&self) -> u64 {
        self.0.into_inner()
    }

    /// Get the named function name
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
    pub fn evaluated_values(&self) -> BTreeMap<u64, f64> {
        self.1
            .evaluated_values()
            .iter()
            .map(|(sample_id, value)| (sample_id.into_inner(), *value))
            .collect()
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
            "SampledNamedFunction(id={}, num_samples={})",
            self.0.into_inner(),
            self.1.evaluated_values().num_samples()
        )
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: Bound<'_, PyAny>) -> Self {
        self.clone()
    }
}
