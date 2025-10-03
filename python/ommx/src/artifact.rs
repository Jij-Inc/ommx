use crate::PyDescriptor;
use anyhow::Result;
use derive_more::{Deref, From};
use ocipkg::{
    image::{Image, OciArchive, OciDir},
    Digest, ImageName,
};
use ommx::artifact::Artifact;
use pyo3::{prelude::*, types::PyBytes};
use std::{collections::HashMap, path::PathBuf, sync::Mutex};

// Import experimental artifact
use ommx::experimental::artifact::Artifact as ExperimentalArtifact;

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
        #[allow(deprecated)]
        let local_path = ommx::artifact::get_image_dir(&image_name);
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

/// Get the current OMMX Local Registry root path.
#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyfunction)]
#[pyfunction]
pub fn get_local_registry_root() -> PathBuf {
    ommx::artifact::get_local_registry_root().to_path_buf()
}

/// Set the OMMX Local Registry root path.
///
/// - The local registry root can be set only once per process,
///   and this function will return an error if it is already set.
/// - The root path is automatically set when used for creating artifacts without calling this function.
/// - Default path is following:
///   - If `OMMX_LOCAL_REGISTRY_ROOT` environment variable is set, its value is used.
///   - Otherwise, OS-specific path by [directories](https://docs.rs/directories/latest/directories/struct.ProjectDirs.html#method.data_dir) is used:
///     - `$XDG_DATA_HOME/ommx/` on Linux
///     - `$HOME/Library/Application Support/org.ommx.ommx/` on macOS
///
#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyfunction)]
#[pyfunction]
pub fn set_local_registry_root(path: PathBuf) -> Result<()> {
    ommx::artifact::set_local_registry_root(path)?;
    Ok(())
}

/// Get the path where given image is stored in the local registry.
///
/// - The directory may not exist if the image is not in the local registry.
///
#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyfunction)]
#[pyfunction]
pub fn get_image_dir(image_name: &str) -> Result<PathBuf> {
    let image_name = ImageName::parse(image_name)?;
    #[allow(deprecated)]
    Ok(ommx::artifact::get_image_dir(&image_name))
}

/// Get the base path for the given image name in the local registry
///
/// This returns the path where the artifact should be stored, without format-specific extensions.
/// The caller should check:
/// - If this path is a directory with oci-layout -> oci-dir format
/// - If "{path}.ommx" exists as a file -> oci-archive format
///
#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyfunction)]
#[pyfunction]
pub fn get_local_registry_path(image_name: &str) -> Result<PathBuf> {
    let image_name = ImageName::parse(image_name)?;
    Ok(ommx::artifact::get_local_registry_path(&image_name))
}


// ============================================================================
// Experimental Artifact API - Using ommx::experimental::artifact
// ============================================================================

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
#[pyo3(module = "ommx._ommx_rust")]
pub struct PyArtifact(Mutex<ExperimentalArtifact>);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl PyArtifact {
    #[staticmethod]
    pub fn from_oci_archive(path: PathBuf) -> Result<Self> {
        let artifact = ExperimentalArtifact::from_oci_archive(&path)?;
        Ok(Self(Mutex::new(artifact)))
    }

    #[staticmethod]
    pub fn from_oci_dir(path: PathBuf) -> Result<Self> {
        let artifact = ExperimentalArtifact::from_oci_dir(&path)?;
        Ok(Self(Mutex::new(artifact)))
    }

    #[staticmethod]
    pub fn from_remote(image_name: &str) -> Result<Self> {
        let image_name = ImageName::parse(image_name)?;
        let artifact = ExperimentalArtifact::from_remote(image_name)?;
        Ok(Self(Mutex::new(artifact)))
    }

    #[staticmethod]
    pub fn load(image_name: &str) -> Result<Self> {
        let image_name = ImageName::parse(image_name)?;
        let artifact = ExperimentalArtifact::load(&image_name)?;
        Ok(Self(Mutex::new(artifact)))
    }

    #[getter]
    pub fn image_name(&mut self) -> Option<String> {
        self.0.lock().unwrap().image_name()
    }

    #[getter]
    pub fn annotations(&mut self) -> Result<HashMap<String, String>> {
        self.0.lock().unwrap().annotations()
    }

    #[getter]
    pub fn layers(&mut self) -> Result<Vec<PyDescriptor>> {
        let layers = self.0.lock().unwrap().layers()?;
        Ok(layers.into_iter().map(PyDescriptor::from).collect())
    }

    pub fn get_blob<'py>(&mut self, py: Python<'py>, digest: &str) -> Result<Bound<'py, PyBytes>> {
        let digest = Digest::new(digest)?;
        let blob = self.0.lock().unwrap().get_blob(&digest)?;
        Ok(PyBytes::new(py, blob.as_ref()))
    }

    pub fn save(&mut self) -> Result<()> {
        self.0.lock().unwrap().save()
    }

    pub fn save_as_archive(&mut self, path: PathBuf) -> Result<()> {
        self.0.lock().unwrap().save_as_archive(&path)
    }

    pub fn save_as_dir(&mut self, path: PathBuf) -> Result<()> {
        self.0.lock().unwrap().save_as_dir(&path)
    }

    pub fn pull(&mut self) -> Result<()> {
        self.0.lock().unwrap().pull()
    }

    pub fn push(&mut self) -> Result<()> {
        self.0.lock().unwrap().push()
    }
}
