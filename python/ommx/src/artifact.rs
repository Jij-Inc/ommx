use crate::PyDescriptor;
use anyhow::Result;
use ocipkg::{Digest, ImageName};
use pyo3::{prelude::*, types::PyBytes};
use std::{collections::HashMap, path::PathBuf, sync::Mutex};

// Import experimental artifact
use ommx::experimental::artifact::Artifact as ExperimentalArtifact;

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


// ============================================================================
// Experimental Builder API
// ============================================================================

use ommx::experimental::artifact::Builder as ExperimentalBuilder;

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
#[pyo3(module = "ommx._ommx_rust")]
pub struct PyArtifactBuilder(Option<ExperimentalBuilder>);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl PyArtifactBuilder {
    #[staticmethod]
    pub fn new_archive(path: PathBuf, image_name: &str) -> Result<Self> {
        let image_name = ImageName::parse(image_name)?;
        let builder = ExperimentalBuilder::new_archive(path, image_name)?;
        Ok(Self(Some(builder)))
    }

    #[staticmethod]
    pub fn new_archive_unnamed(path: PathBuf) -> Result<Self> {
        let builder = ExperimentalBuilder::new_archive_unnamed(path)?;
        Ok(Self(Some(builder)))
    }

    #[staticmethod]
    pub fn temp_archive() -> Result<Self> {
        let builder = ExperimentalBuilder::temp_archive()?;
        Ok(Self(Some(builder)))
    }

    #[staticmethod]
    pub fn new_dir(path: PathBuf, image_name: &str) -> Result<Self> {
        let image_name = ImageName::parse(image_name)?;
        let builder = ExperimentalBuilder::new_dir(path, image_name)?;
        Ok(Self(Some(builder)))
    }

    pub fn add_annotation(&mut self, key: String, value: String) {
        if let Some(builder) = &mut self.0 {
            builder.add_annotation(key, value);
        }
    }


    pub fn add_layer(&mut self, media_type: &str, blob: &Bound<PyBytes>, annotations: HashMap<String, String>) -> Result<PyDescriptor> {
        use ocipkg::distribution::MediaType;

        let media_type = MediaType::Other(media_type.to_string());

        let builder = self.0.as_mut().ok_or_else(|| anyhow::anyhow!("Builder already consumed"))?;

        let blob_bytes = blob.as_bytes();

        let desc = match builder {
            ExperimentalBuilder::Archive(b) => {
                b.add_layer(media_type, blob_bytes, annotations)?
            }
            ExperimentalBuilder::Dir(b) => {
                b.add_layer(media_type, blob_bytes, annotations)?
            }
        };
        Ok(PyDescriptor::from(desc))
    }

    pub fn build(&mut self) -> Result<PyArtifact> {
        let builder = self.0.take().ok_or_else(|| anyhow::anyhow!("Builder already consumed"))?;
        let artifact = builder.build()?;
        Ok(PyArtifact(Mutex::new(artifact)))
    }
}
