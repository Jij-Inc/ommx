use approx::AbsDiffEq;
use ommx::{v1, Message};
use pyo3::{exceptions::PyRuntimeError, prelude::*, types::PyBytes};

#[pyclass]
pub struct Linear(v1::Linear);

#[pymethods]
impl Linear {
    #[staticmethod]
    pub fn decode(bytes: &Bound<PyBytes>) -> PyResult<Self> {
        let inner = v1::Linear::decode(bytes.as_bytes())
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(Self(inner))
    }

    pub fn encode<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let bytes = self.0.encode_to_vec();
        Ok(PyBytes::new_bound(py, &bytes))
    }

    pub fn almost_equal(&self, other: &Linear, atol: f64) -> bool {
        self.0.abs_diff_eq(&other.0, atol)
    }
}
