use pyo3::prelude::*;
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};
use std::collections::{BTreeSet, HashMap};

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

/// A removed SOS1 constraint together with the reason it was removed.
#[gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct RemovedSos1Constraint {
    pub constraint: ommx::Sos1Constraint,
    pub removed_reason: ommx::RemovedReason,
}

impl RemovedSos1Constraint {
    pub fn from_pair(
        constraint: ommx::Sos1Constraint,
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
impl RemovedSos1Constraint {
    #[getter]
    pub fn constraint(&self) -> Sos1Constraint {
        Sos1Constraint(self.constraint.clone())
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
        format!("RemovedSos1Constraint({head})")
    }
}
