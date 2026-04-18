use pyo3::prelude::*;
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};
use std::collections::BTreeSet;

/// A one-hot constraint: exactly one variable must be 1, the rest must be 0.
///
/// This is a structural constraint — no explicit function is stored.
/// The implicit constraint is `sum(x_i) = 1` where all `x_i` are binary.
#[gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct OneHotConstraint(pub ommx::OneHotConstraint);

#[gen_stub_pymethods]
#[pymethods]
impl OneHotConstraint {
    /// Create a new one-hot constraint.
    ///
    /// **Args:**
    ///
    /// - `variables`: List of binary decision variable IDs (exactly one must be 1)
    #[new]
    #[pyo3(signature = (*, variables))]
    pub fn new(variables: Vec<u64>) -> Self {
        let vars: BTreeSet<ommx::VariableID> =
            variables.into_iter().map(ommx::VariableID::from).collect();
        Self(ommx::OneHotConstraint::new(vars))
    }

    #[getter]
    pub fn variables(&self) -> Vec<u64> {
        self.0.variables.iter().map(|v| v.into_inner()).collect()
    }

    fn __repr__(&self) -> String {
        format!("{}", self.0)
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: pyo3::Bound<pyo3::types::PyAny>) -> Self {
        self.clone()
    }
}
