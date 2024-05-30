use crate::PyDescriptor;
use anyhow::Result;
use derive_more::{Deref, From};
use ocipkg::{
    image::{Image, OciArchive, OciDir},
    Digest, ImageName,
};
use ommx::artifact::{image_dir, Artifact};
use pyo3::{prelude::*, types::PyBytes};
use std::{collections::HashMap, path::PathBuf};

#[pyclass]
#[pyo3(module = "ommx._ommx_rust")]
#[derive(From, Deref)]
pub struct ArtifactArchive(Artifact<OciArchive>);

#[pymethods]
impl ArtifactArchive {
    #[staticmethod]
    pub fn from_oci_archive(path: PathBuf) -> Result<Self> {
        let artifact = Artifact::from_oci_archive(&path)?;
        Ok(Self(artifact))
    }

    #[getter]
    pub fn annotations(&mut self) -> Result<HashMap<String, String>> {
        let manifest = self.0.get_manifest()?;
        Ok(manifest.annotations().as_ref().cloned().unwrap_or_default())
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
#[derive(From, Deref)]
pub struct ArtifactDir(Artifact<OciDir>);

#[pymethods]
impl ArtifactDir {
    #[staticmethod]
    pub fn from_image_name(image_name: &str) -> Result<Self> {
        let image_name = ImageName::parse(image_name)?;
        let local_path = image_dir(&image_name)?;
        if local_path.exists() {
            return Ok(Self(Artifact::from_oci_dir(&local_path)?));
        }
        let mut remote = Artifact::from_remote(image_name)?;
        Ok(Self(remote.pull()?))
    }

    #[staticmethod]
    pub fn from_oci_dir(path: PathBuf) -> Result<Self> {
        let artifact = Artifact::from_oci_dir(&path)?;
        Ok(Self(artifact))
    }

    #[getter]
    pub fn annotations(&mut self) -> Result<HashMap<String, String>> {
        let manifest = self.0.get_manifest()?;
        Ok(manifest.annotations().as_ref().cloned().unwrap_or_default())
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
