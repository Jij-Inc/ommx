use pyo3::prelude::*;
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};
use std::collections::{BTreeSet, HashMap};

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

/// A removed one-hot constraint together with the reason it was removed.
#[gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct RemovedOneHotConstraint {
    pub constraint: ommx::OneHotConstraint,
    pub removed_reason: ommx::RemovedReason,
}

impl RemovedOneHotConstraint {
    pub fn from_pair(
        constraint: ommx::OneHotConstraint,
        removed_reason: ommx::RemovedReason,
    ) -> Self {
        Self {
            constraint,
            removed_reason,
        }
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl RemovedOneHotConstraint {
    #[getter]
    pub fn constraint(&self) -> OneHotConstraint {
        OneHotConstraint(self.constraint.clone())
    }

    #[getter]
    pub fn variables(&self) -> Vec<u64> {
        self.constraint
            .variables
            .iter()
            .map(|v| v.into_inner())
            .collect()
    }

    #[getter]
    pub fn removed_reason(&self) -> String {
        self.removed_reason.reason.clone()
    }

    #[getter]
    pub fn removed_reason_parameters(&self) -> HashMap<String, String> {
        self.removed_reason
            .parameters
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    fn __repr__(&self) -> String {
        let mut extras: Vec<String> = self
            .removed_reason
            .parameters
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        extras.sort();
        let mut head = format!("{}, reason={}", self.constraint, self.removed_reason.reason);
        if !extras.is_empty() {
            head.push_str(", ");
            head.push_str(&extras.join(", "));
        }
        format!("RemovedOneHotConstraint({head})")
    }
}
