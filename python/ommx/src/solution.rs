use anyhow::Result;
use ommx::{Message, Parse};
use pyo3::{prelude::*, types::PyBytes, Bound};

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
}
