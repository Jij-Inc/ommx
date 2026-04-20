use pyo3::prelude::*;
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};
use std::collections::BTreeSet;

/// A SOS1 (Special Ordered Set type 1) constraint: at most one variable can be non-zero.
///
/// This is a structural constraint — no explicit function is stored.
/// Unlike OneHotConstraint, SOS1 allows all variables to be zero.
#[gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct Sos1Constraint(pub ommx::Sos1Constraint);

#[gen_stub_pymethods]
#[pymethods]
impl Sos1Constraint {
    /// Create a new SOS1 constraint.
    ///
    /// **Args:**
    ///
    /// - `variables`: List of decision variable IDs (at most one can be non-zero)
    #[new]
    #[pyo3(signature = (*, variables))]
    pub fn new(variables: Vec<u64>) -> Self {
        let vars: BTreeSet<ommx::VariableID> =
            variables.into_iter().map(ommx::VariableID::from).collect();
        Self(ommx::Sos1Constraint::new(vars))
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
