use crate::Descriptor;
use anyhow::Result;
use ocipkg::image::OciArchive;
use ommx::artifact::media_types;
use pyo3::prelude::*;
use std::path::PathBuf;

#[pyclass]
#[pyo3(module = "ommx._ommx_rust")]
pub struct Artifact(ommx::artifact::Artifact<OciArchive>);

#[pymethods]
impl Artifact {
    #[staticmethod]
    pub fn from_oci_archive(path: PathBuf) -> Result<Self> {
        let artifact = ommx::artifact::Artifact::from_oci_archive(&path)?;
        Ok(Self(artifact))
    }

    #[getter]
    pub fn instance_descriptors(&mut self) -> Result<Vec<Descriptor>> {
        self.0
            .get_layer_descriptors(&media_types::v1_instance())
            .map(|descs| descs.into_iter().map(Descriptor::from).collect())
    }
}
