use crate::PyDescriptor;
use anyhow::Result;
use derive_more::{Deref, From};
use ocipkg::{
    image::{Image, OciArchive, OciDir},
    Digest, ImageName,
};
use ommx::artifact::{image_dir, Artifact};
use pyo3::{prelude::*, types::PyBytes};
use std::{collections::HashMap, path::PathBuf, sync::Mutex};

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
#[pyo3(module = "ommx._ommx_rust")]
#[derive(From, Deref)]
pub struct ArtifactArchive(Mutex<Artifact<OciArchive>>);

impl From<Artifact<OciArchive>> for ArtifactArchive {
    fn from(artifact: Artifact<OciArchive>) -> Self {
        Self(Mutex::new(artifact))
    }
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl ArtifactArchive {
    #[staticmethod]
    pub fn from_oci_archive(path: PathBuf) -> Result<Self> {
        let artifact = Artifact::from_oci_archive(&path)?;
        Ok(Self(artifact.into()))
    }

    #[getter]
    pub fn image_name(&mut self) -> Option<String> {
        self.0
            .lock()
            .unwrap()
            .get_name()
            .map(|name| name.to_string())
            .ok()
    }

    #[getter]
    pub fn annotations(&mut self) -> Result<HashMap<String, String>> {
        let manifest = self.0.lock().unwrap().get_manifest()?;
        Ok(manifest.annotations().as_ref().cloned().unwrap_or_default())
    }

    #[getter]
    pub fn layers(&mut self) -> Result<Vec<PyDescriptor>> {
        let manifest = self.0.lock().unwrap().get_manifest()?;
        Ok(manifest
            .layers()
            .iter()
            .cloned()
            .map(PyDescriptor::from)
            .collect())
    }

    pub fn get_blob<'py>(&mut self, py: Python<'py>, digest: &str) -> Result<Bound<'py, PyBytes>> {
        let digest = Digest::new(digest)?;
        let blob = self.0.lock().unwrap().get_blob(&digest)?;
        Ok(PyBytes::new(py, blob.as_ref()))
    }

    pub fn push(&mut self) -> Result<()> {
        // Do not expose Artifact<Remote> to Python API for simplicity.
        // In Python API, the `Artifact` class always refers to the local artifact, which may be either an OCI archive or an OCI directory.
        let _remote = self.0.lock().unwrap().push()?;
        Ok(())
    }
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
#[pyo3(module = "ommx._ommx_rust")]
#[derive(From, Deref)]
pub struct ArtifactDir(Artifact<OciDir>);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
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
    pub fn image_name(&mut self) -> Option<String> {
        self.0.get_name().map(|name| name.to_string()).ok()
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
        Ok(PyBytes::new(py, blob.as_ref()))
    }

    pub fn push(&mut self) -> Result<()> {
        // Do not expose Artifact<Remote> to Python API for simplicity.
        // In Python API, the `Artifact` class always refers to the local artifact, which may be either an OCI archive or an OCI directory.
        let _remote = self.0.push()?;
        Ok(())
    }
}
