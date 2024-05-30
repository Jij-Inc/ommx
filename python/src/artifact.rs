use crate::PyDescriptor;
use anyhow::Result;
use ocipkg::{
    image::{Image, OciArchive, OciDir},
    Digest,
};
use pyo3::{prelude::*, types::PyBytes};
use std::path::PathBuf;

#[pyclass]
#[pyo3(module = "ommx._ommx_rust")]
pub struct ArtifactArchive(ommx::artifact::Artifact<OciArchive>);

#[pymethods]
impl ArtifactArchive {
    #[staticmethod]
    pub fn from_oci_archive(path: PathBuf) -> Result<Self> {
        let artifact = ommx::artifact::Artifact::from_oci_archive(&path)?;
        Ok(Self(artifact))
    }

    #[getter]
    pub fn layers(&mut self) -> Result<Vec<PyDescriptor>> {
        let manifest = self.0.get_manifest()?;
        Ok(manifest
            .layers()
            .iter()
            .cloned()
            .map(PyDescriptor::from)
            .collect())
    }

    pub fn get_blob<'py>(&mut self, py: Python<'py>, digest: &str) -> Result<Bound<'py, PyBytes>> {
        let digest = Digest::new(digest)?;
        let blob = self.0.get_blob(&digest)?;
        Ok(PyBytes::new_bound(py, blob.as_ref()))
    }
}

#[pyclass]
#[pyo3(module = "ommx._ommx_rust")]
pub struct ArtifactDir(ommx::artifact::Artifact<OciDir>);

#[pymethods]
impl ArtifactDir {
    #[staticmethod]
    pub fn from_oci_dir(path: PathBuf) -> Result<Self> {
        let artifact = ommx::artifact::Artifact::from_oci_dir(&path)?;
        Ok(Self(artifact))
    }

    #[getter]
    pub fn layers(&mut self) -> Result<Vec<PyDescriptor>> {
        let manifest = self.0.get_manifest()?;
        Ok(manifest
            .layers()
            .iter()
            .cloned()
            .map(PyDescriptor::from)
            .collect())
    }

    pub fn get_blob<'py>(&mut self, py: Python<'py>, digest: &str) -> Result<Bound<'py, PyBytes>> {
        let digest = Digest::new(digest)?;
        let blob = self.0.get_blob(&digest)?;
        Ok(PyBytes::new_bound(py, blob.as_ref()))
    }
}
