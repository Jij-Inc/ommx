use crate::{Function, Instance, Parameters};
use anyhow::Result;
use ommx::VariableID;
use pyo3::{exceptions::PyValueError, prelude::*, types::PyBytes, Bound};
use std::collections::HashMap;

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct ParametricInstance(pub ommx::ParametricInstance);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl ParametricInstance {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        let inner = ommx::ParametricInstance::from_bytes(bytes.as_bytes())?;
        Ok(Self(inner))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.0.to_bytes())
    }

    pub fn with_parameters(&self, parameters: &Parameters) -> Result<Instance> {
        let instance = self.0.clone().with_parameters(parameters.0.clone())?;
        Ok(Instance(instance))
    }

    #[pyo3(signature = (assignments))]
    pub fn substitute(&mut self, assignments: HashMap<u64, Function>) -> PyResult<()> {
        let iter = assignments
            .into_iter()
            .map(|(id, f)| (VariableID::from(id), f.0));
        ommx::substitute(&mut self.0, iter).map_err(|e| PyValueError::new_err(e.to_string()))?;
        Ok(())
    }
}
