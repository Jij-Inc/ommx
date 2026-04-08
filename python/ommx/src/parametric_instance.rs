use crate::{Instance, Parameters};
use anyhow::Result;
use pyo3::{prelude::*, types::PyBytes, Bound};
use std::collections::HashMap;

#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
pub struct ParametricInstance {
    pub(crate) inner: ommx::ParametricInstance,
    pub(crate) annotations: HashMap<String, String>,
}

crate::annotations::impl_instance_annotations!(
    ParametricInstance,
    "org.ommx.v1.parametric-instance"
);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl ParametricInstance {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        let inner = ommx::ParametricInstance::from_bytes(bytes.as_bytes())?;
        Ok(Self {
            inner,
            annotations: HashMap::new(),
        })
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.inner.to_bytes())
    }

    pub fn with_parameters(&self, parameters: &Parameters) -> Result<Instance> {
        let instance = self.inner.clone().with_parameters(parameters.0.clone())?;
        Ok(Instance {
            inner: instance,
            annotations: HashMap::new(),
        })
    }
}
