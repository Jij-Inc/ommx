use anyhow::Result;
use ommx::{Message, Parse};
use pyo3::{
    exceptions::PyKeyError,
    prelude::*,
    types::{PyBytes, PyDict, PyTuple},
    Bound,
};
use std::collections::{BTreeMap, BTreeSet};

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct Solution(pub ommx::Solution);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl Solution {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        let v1_inner = ommx::v1::Solution::decode(bytes.as_bytes())?;
        let inner = v1_inner.parse(&())?;
        Ok(Self(inner))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let v1_solution: ommx::v1::Solution = self.0.clone().into();
        Ok(PyBytes::new(py, &v1_solution.encode_to_vec()))
    }

    /// Get the objective function value
    #[getter]
    pub fn objective(&self) -> f64 {
        *self.0.objective()
    }

    /// Get the solution state containing variable values
    #[getter]
    pub fn state(&self) -> crate::State {
        crate::State(self.0.state())
    }

    /// Check if the solution is feasible
    #[getter]
    pub fn feasible(&self) -> bool {
        // The `feasible` means feasible in the unrelaxed problem
        self.0.feasible()
    }

    /// Check if the solution is feasible in the relaxed problem
    #[getter]
    pub fn feasible_relaxed(&self) -> bool {
        self.0.feasible_relaxed()
    }

    /// Check if the solution is feasible in the unrelaxed problem  
    #[getter]
    pub fn feasible_unrelaxed(&self) -> bool {
        self.0.feasible()
    }

    /// Get the optimality status
    #[getter]
    pub fn optimality(&self) -> crate::Optimality {
        self.0.optimality.into()
    }

    /// Get the relaxation status
    #[getter]
    pub fn relaxation(&self) -> crate::Relaxation {
        self.0.relaxation.into()
    }

    /// Set the optimality status
    #[setter]
    pub fn set_optimality(&mut self, optimality: crate::Optimality) {
        self.0.optimality = optimality.into();
    }

    /// Set the relaxation status
    #[setter]
    pub fn set_relaxation(&mut self, relaxation: crate::Relaxation) {
        self.0.relaxation = relaxation.into();
    }

    /// Get decision variables information as a map from ID to EvaluatedDecisionVariable
    #[getter]
    pub fn decision_variables(&self) -> BTreeMap<u64, crate::EvaluatedDecisionVariable> {
        self.0
            .decision_variables()
            .iter()
            .map(|(id, dv)| {
                (
                    id.into_inner(),
                    crate::EvaluatedDecisionVariable(dv.clone()),
                )
            })
            .collect()
    }

    /// Get evaluated constraints information as a map from ID to EvaluatedConstraint
    #[getter]
    pub fn evaluated_constraints(&self) -> BTreeMap<u64, crate::EvaluatedConstraint> {
        self.0
            .evaluated_constraints()
            .iter()
            .map(|(id, ec)| (id.into_inner(), crate::EvaluatedConstraint(ec.clone())))
            .collect()
    }

    #[getter]
    pub fn decision_variable_ids(&self) -> BTreeSet<u64> {
        self.0
            .decision_variable_ids()
            .iter()
            .map(|id| id.into_inner())
            .collect()
    }

    #[getter]
    pub fn constraint_ids(&self) -> BTreeSet<u64> {
        self.0
            .constraint_ids()
            .iter()
            .map(|id| id.into_inner())
            .collect()
    }

    /// Extract decision variables by name with subscripts as key (returns a Python dict)
    pub fn extract_decision_variables<'py>(
        &self,
        py: Python<'py>,
        name: &str,
    ) -> Result<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        for (subscripts, value) in self.0.extract_decision_variables(name)? {
            let key_tuple = PyTuple::new(py, &subscripts)?;
            dict.set_item(key_tuple, value)?;
        }
        Ok(dict)
    }

    /// Extract constraints by name with subscripts as key (returns a Python dict)
    pub fn extract_constraints<'py>(
        &self,
        py: Python<'py>,
        name: &str,
    ) -> Result<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        for (subscripts, value) in self.0.extract_constraints(name)? {
            let key_tuple = PyTuple::new(py, &subscripts)?;
            dict.set_item(key_tuple, value)?;
        }
        Ok(dict)
    }

    /// Get the evaluated value of a specific constraint by ID
    pub fn get_constraint_value(&self, constraint_id: u64) -> PyResult<f64> {
        let constraint_id = ommx::ConstraintID::from(constraint_id);
        self.0
            .get_constraint_value(constraint_id)
            .map_err(|e| PyKeyError::new_err(e.to_string()))
    }

    /// Get the dual variable value of a specific constraint by ID
    pub fn get_dual_variable(&self, constraint_id: u64) -> PyResult<Option<f64>> {
        let constraint_id = ommx::ConstraintID::from(constraint_id);
        self.0
            .get_dual_variable(constraint_id)
            .map_err(|e| PyKeyError::new_err(e.to_string()))
    }

    /// Set the dual variable value for a specific constraint by ID
    pub fn set_dual_variable(&mut self, constraint_id: u64, value: Option<f64>) -> PyResult<()> {
        let constraint_id = ommx::ConstraintID::from(constraint_id);
        self.0
            .set_dual_variable(constraint_id, value)
            .map_err(|e| PyKeyError::new_err(e.to_string()))
    }
}
