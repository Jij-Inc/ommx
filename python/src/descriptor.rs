use ocipkg::oci_spec::image::Descriptor as RawDescriptor;
use pyo3::prelude::*;
use std::collections::HashMap;

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
