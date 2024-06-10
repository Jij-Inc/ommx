use anyhow::Result;
use derive_more::{Deref, From};
use ocipkg::oci_spec::image::Descriptor;
use pyo3::{prelude::*, types::PyDict};
use std::collections::HashMap;

/// Descriptor of blob in artifact
#[pyclass]
#[pyo3(module = "ommx._ommx_rust", name = "Descriptor")]
#[derive(Debug, Clone, PartialEq, From, Deref)]
pub struct PyDescriptor(Descriptor);

#[pymethods]
impl PyDescriptor {
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

    /// Return annotations with key prefix "org.ommx.user."
    #[getter]
    pub fn user_annotations(&self) -> HashMap<String, String> {
        if let Some(annotations) = self.0.annotations() {
            annotations
                .iter()
                .flat_map(|(k, v)| {
                    k.strip_prefix("org.ommx.user.")
                        .map(|k| (k.to_string(), v.clone()))
                })
                .collect()
        } else {
            HashMap::new()
        }
    }
}
