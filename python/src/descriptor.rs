use anyhow::Result;
use ocipkg::oci_spec::image::Descriptor as RawDescriptor;
use pyo3::{prelude::*, types::PyDict};
use std::collections::HashMap;

/// Descriptor of blob in artifact
#[pyclass]
#[pyo3(module = "ommx._ommx_rust")]
pub struct Descriptor(RawDescriptor);

impl From<RawDescriptor> for Descriptor {
    fn from(descriptor: RawDescriptor) -> Self {
        Self(descriptor)
    }
}

#[pymethods]
impl Descriptor {
    pub fn to_dict<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyDict>> {
        let any = serde_pyobject::to_pyobject(py, &self.0)?;
        Ok(any.extract()?)
    }

    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(&self.0)?)
    }

    pub fn __str__(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(&self.0)?)
    }

    #[getter]
    pub fn digest(&self) -> &str {
        self.0.digest()
    }

    #[getter]
    pub fn size(&self) -> i64 {
        self.0.size()
    }

    #[getter]
    pub fn media_type(&self) -> String {
        self.0.media_type().to_string()
    }

    #[getter]
    pub fn annotations(&self) -> HashMap<String, String> {
        if let Some(annotations) = self.0.annotations() {
            annotations.clone()
        } else {
            HashMap::new()
        }
    }
}
