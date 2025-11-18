use anyhow::Result;
use derive_more::{Deref, From};
use ocipkg::oci_spec::image::Descriptor;
use pyo3::{prelude::*, types::PyDict};
use std::collections::HashMap;

/// Descriptor of blob in artifact
#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
#[pyo3(module = "ommx._ommx_rust", name = "Descriptor")]
#[derive(Debug, Clone, PartialEq, From, Deref)]
pub struct PyDescriptor(Descriptor);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl PyDescriptor {
    pub fn to_dict<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyDict>> {
        let any = serde_pyobject::to_pyobject(py, &self.0)?;
        any.extract().map_err(|e| anyhow::anyhow!("{}", e))
    }

    #[staticmethod]
    pub fn from_dict(dict: Bound<PyDict>) -> Result<Self> {
        let descriptor = serde_pyobject::from_pyobject(dict)?;
        Ok(Self(descriptor))
    }

    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(&self.0)?)
    }

    #[staticmethod]
    pub fn from_json(json: &str) -> Result<Self> {
        let descriptor = serde_json::from_str(json)?;
        Ok(Self(descriptor))
    }

    pub fn __str__(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(&self.0)?)
    }

    pub fn __eq__(&self, rhs: &Bound<PyAny>) -> bool {
        let Ok(rhs) = rhs.extract::<Self>() else {
            return false;
        };
        self.0 == rhs.0
    }

    #[getter]
    pub fn digest(&self) -> String {
        self.0.digest().to_string()
    }

    #[getter]
    pub fn size(&self) -> i64 {
        self.0.size() as i64
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
