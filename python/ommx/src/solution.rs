use anyhow::Result;
use ommx::{Message, Parse};
use pyo3::{
    exceptions::PyKeyError,
    prelude::*,
    types::{PyBytes, PyDict, PyTuple},
    Bound,
};
use std::collections::BTreeSet;

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

    /// Get evaluated decision variables as a list sorted by ID
    #[getter]
    pub fn decision_variables(&self) -> Vec<crate::EvaluatedDecisionVariable> {
        // BTreeMap is already sorted by key
        self.0
            .decision_variables()
            .values()
            .map(|dv| crate::EvaluatedDecisionVariable(dv.clone()))
            .collect()
    }

    /// Get evaluated constraints as a list sorted by ID
    #[getter]
    pub fn constraints(&self) -> Vec<crate::EvaluatedConstraint> {
        // BTreeMap is already sorted by key
        self.0
            .evaluated_constraints()
            .values()
            .map(|ec| crate::EvaluatedConstraint(ec.clone()))
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

    /// Set the dual variable value for a specific constraint by ID
    pub fn set_dual_variable(&mut self, constraint_id: u64, value: Option<f64>) -> PyResult<()> {
        let constraint_id = ommx::ConstraintID::from(constraint_id);
        self.0
            .set_dual_variable(constraint_id, value)
            .map_err(|e| PyKeyError::new_err(e.to_string()))
    }

    /// Get a specific evaluated decision variable by ID
    pub fn get_decision_variable_by_id(
        &self,
        variable_id: u64,
    ) -> PyResult<crate::EvaluatedDecisionVariable> {
        let var_id = ommx::VariableID::from(variable_id);
        self.0
            .decision_variables()
            .get(&var_id)
            .map(|dv| crate::EvaluatedDecisionVariable(dv.clone()))
            .ok_or_else(|| {
                PyKeyError::new_err(format!("Unknown decision variable ID: {variable_id}"))
            })
    }

    /// Get a specific evaluated constraint by ID
    pub fn get_constraint_by_id(&self, constraint_id: u64) -> PyResult<crate::EvaluatedConstraint> {
        let constraint_id = ommx::ConstraintID::from(constraint_id);
        self.0
            .evaluated_constraints()
            .get(&constraint_id)
            .map(|ec| crate::EvaluatedConstraint(ec.clone()))
            .ok_or_else(|| {
                PyKeyError::new_err(format!(
                    "Unknown constraint ID: {}",
                    constraint_id.into_inner()
                ))
            })
    }
}
