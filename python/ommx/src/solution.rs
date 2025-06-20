use anyhow::Result;
use ommx::{Message, Parse};
use pyo3::{prelude::*, types::{PyBytes, PyTuple, PyDict}, Bound, exceptions::PyValueError};
use std::collections::BTreeMap;

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

    /// Get decision variables information as a map from ID to EvaluatedDecisionVariable
    #[getter]
    pub fn decision_variables(&self) -> BTreeMap<u64, crate::EvaluatedDecisionVariable> {
        self.0.decision_variables()
            .iter()
            .map(|(id, dv)| (id.into_inner(), crate::EvaluatedDecisionVariable(dv.clone())))
            .collect()
    }

    /// Get evaluated constraints information as a map from ID to EvaluatedConstraint
    #[getter]
    pub fn evaluated_constraints(&self) -> BTreeMap<u64, crate::EvaluatedConstraint> {
        self.0.evaluated_constraints()
            .iter()
            .map(|(id, ec)| (id.into_inner(), crate::EvaluatedConstraint(ec.clone())))
            .collect()
    }

    /// Extract decision variables by name with subscripts as key (returns a Python dict)
    pub fn extract_decision_variables<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyDict>> {
        let result_dict = PyDict::new(py);
        
        for (_, dv) in self.0.decision_variables() {
            if let Some(dv_name) = &dv.metadata.name {
                if dv_name == name {
                    if !dv.metadata.parameters.is_empty() {
                        return Err(PyValueError::new_err("Decision variable with parameters is not supported"));
                    }
                    let key_tuple = PyTuple::new(py, &dv.metadata.subscripts)?;
                    if result_dict.contains(&key_tuple)? {
                        return Err(PyValueError::new_err(format!("Duplicate subscript: {:?}", dv.metadata.subscripts)));
                    }
                    result_dict.set_item(key_tuple, *dv.value())?;
                }
            }
        }
        Ok(result_dict)
    }

    /// Extract constraints by name with subscripts as key (returns a Python dict)
    pub fn extract_constraints<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyDict>> {
        let result_dict = PyDict::new(py);
        
        for (_, ec) in self.0.evaluated_constraints() {
            if let Some(constraint_name) = &ec.metadata.name {
                if constraint_name == name {
                    if !ec.metadata.parameters.is_empty() {
                        return Err(PyValueError::new_err("Constraint with parameters is not supported"));
                    }
                    let key_tuple = PyTuple::new(py, &ec.metadata.subscripts)?;
                    if result_dict.contains(&key_tuple)? {
                        return Err(PyValueError::new_err(format!("Duplicate subscript: {:?}", ec.metadata.subscripts)));
                    }
                    result_dict.set_item(key_tuple, *ec.evaluated_value())?;
                }
            }
        }
        Ok(result_dict)
    }
}
