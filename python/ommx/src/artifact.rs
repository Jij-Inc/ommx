use anyhow::{bail, Result};
use ocipkg::image::{Image, OciArchive, OciDir};
use ommx::artifact::Artifact;
use pyo3::{prelude::*, types::PyBytes};
use std::{collections::HashMap, path::PathBuf, sync::Mutex};

use crate::PyDescriptor;

// ---------------------------------------------------------------------------
// ArtifactInner: unified wrapper for Archive / Dir
// ---------------------------------------------------------------------------

enum ArtifactInner {
    Archive(Mutex<Artifact<OciArchive>>),
    Dir(Artifact<OciDir>),
    Local(Box<Mutex<ommx::artifact::LocalArtifact>>),
}

impl ArtifactInner {
    fn image_name(&mut self) -> Option<String> {
        match self {
            ArtifactInner::Archive(a) => {
                a.get_mut().unwrap().get_name().map(|n| n.to_string()).ok()
            }
            ArtifactInner::Dir(d) => d.get_name().map(|n| n.to_string()).ok(),
            ArtifactInner::Local(local) => Some(local.get_mut().unwrap().image_name().to_string()),
        }
    }

    fn annotations(&mut self) -> Result<HashMap<String, String>> {
        match self {
            ArtifactInner::Archive(a) => {
                let manifest = a.get_mut().unwrap().get_manifest()?;
                Ok(manifest.annotations().as_ref().cloned().unwrap_or_default())
            }
            ArtifactInner::Dir(d) => {
                let manifest = d.get_manifest()?;
                Ok(manifest.annotations().as_ref().cloned().unwrap_or_default())
            }
            ArtifactInner::Local(local) => local.get_mut().unwrap().annotations(),
        }
    }

    fn layers(&mut self) -> Result<Vec<PyDescriptor>> {
        match self {
            ArtifactInner::Archive(a) => {
                let manifest = a.get_mut().unwrap().get_manifest()?;
                Ok(manifest
                    .layers()
                    .iter()
                    .cloned()
                    .map(PyDescriptor::from)
                    .collect())
            }
            ArtifactInner::Dir(d) => {
                let manifest = d.get_manifest()?;
                Ok(manifest
                    .layers()
                    .iter()
                    .cloned()
                    .map(PyDescriptor::from)
                    .collect())
            }
            ArtifactInner::Local(local) => Ok(local
                .get_mut()
                .unwrap()
                .layers()?
                .into_iter()
                .map(PyDescriptor::from)
                .collect()),
        }
    }

    fn get_blob(&mut self, digest: &str) -> Result<Vec<u8>> {
        match self {
            ArtifactInner::Archive(a) => {
                let digest = digest.parse()?;
                let blob = a.get_mut().unwrap().get_blob(&digest)?;
                Ok(blob.to_vec())
            }
            ArtifactInner::Dir(d) => {
                let digest = digest.parse()?;
                let blob = d.get_blob(&digest)?;
                Ok(blob.to_vec())
            }
            ArtifactInner::Local(local) => local.get_mut().unwrap().get_blob(digest),
        }
    }

    #[cfg(feature = "remote-artifact")]
    fn push(&mut self) -> Result<()> {
        match self {
            ArtifactInner::Archive(a) => {
                let _remote = a.get_mut().unwrap().push()?;
                Ok(())
            }
            ArtifactInner::Dir(d) => {
                let _remote = d.push()?;
                Ok(())
            }
            ArtifactInner::Local(_) => {
                bail!("Pushing SQLite-backed local registry artifacts is not implemented yet")
            }
        }
    }
}

// ---------------------------------------------------------------------------
// PyArtifact
// ---------------------------------------------------------------------------

/// Reader for OMMX Artifacts.
///
/// An artifact is an OCI container image that stores OMMX data
/// (instances, solutions, sample sets, etc.) as layers.
///
/// ```python
/// >>> artifact = Artifact.load("ghcr.io/jij-inc/ommx/random_lp_instance:4303c7f")
/// >>> print(artifact.image_name)
/// ghcr.io/jij-inc/ommx/random_lp_instance:4303c7f
///
/// ```
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[pyo3(module = "ommx._ommx_rust", name = "Artifact")]
pub struct PyArtifact(ArtifactInner);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl PyArtifact {
    /// Load an artifact stored as a single file or directory.
    ///
    /// ```python
    /// >>> artifact = Artifact.load_archive("data/random_lp_instance.ommx")
    /// >>> print(artifact.image_name)
    /// ghcr.io/jij-inc/ommx/random_lp_instance:...
    ///
    /// ```
    #[staticmethod]
    pub fn load_archive(py: Python<'_>, path: PathBuf) -> Result<Self> {
        let _guard = crate::TRACING.attach_parent_context(py);
        if path.is_file() {
            let artifact = Artifact::from_oci_archive(&path)?;
            Ok(Self(ArtifactInner::Archive(Mutex::new(artifact))))
        } else if path.is_dir() {
            let artifact = Artifact::from_oci_dir(&path)?;
            Ok(Self(ArtifactInner::Dir(artifact)))
        } else {
            bail!("Path must be a file or a directory")
        }
    }

    /// Load an artifact stored as a container image in local or remote registry.
    ///
    /// If the image is not found in local registry, it will try to pull from remote registry.
    ///
    /// ```python
    /// >>> artifact = Artifact.load("ghcr.io/jij-inc/ommx/random_lp_instance:4303c7f")
    /// >>> print(artifact.image_name)
    /// ghcr.io/jij-inc/ommx/random_lp_instance:4303c7f
    ///
    /// ```
    #[cfg(feature = "remote-artifact")]
    #[staticmethod]
    pub fn load(py: Python<'_>, image_name: &str) -> Result<Self> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let image_name_parsed = ocipkg::ImageName::parse(image_name)?;
        if let Some(artifact) = ommx::artifact::LocalArtifact::try_open(image_name_parsed.clone())?
        {
            return Ok(Self(ArtifactInner::Local(Box::new(Mutex::new(artifact)))));
        }
        let local_path = ommx::artifact::get_image_dir(&image_name_parsed);
        if local_path.exists() {
            bail!(
                "Artifact {image_name} was found only in the legacy OCI directory local registry at {}. \
                 Run `ommx artifact migrate` once, then retry.",
                local_path.display()
            );
        }
        let mut remote = Artifact::from_remote(image_name_parsed)?;
        let local = remote.pull()?;
        Ok(Self(ArtifactInner::Dir(local)))
    }

    /// Push the artifact to remote registry.
    #[cfg(feature = "remote-artifact")]
    pub fn push(&mut self, py: Python<'_>) -> Result<()> {
        let _guard = crate::TRACING.attach_parent_context(py);
        self.0.push()
    }

    #[getter]
    pub fn image_name(&mut self) -> Option<String> {
        self.0.image_name()
    }

    /// Annotations in the artifact manifest.
    #[getter]
    pub fn annotations(&mut self) -> Result<HashMap<String, String>> {
        self.0.annotations()
    }

    #[getter]
    pub fn layers(&mut self) -> Result<Vec<PyDescriptor>> {
        self.0.layers()
    }

    /// Look up a layer descriptor by digest.
    pub fn get_layer_descriptor(&mut self, py: Python<'_>, digest: &str) -> Result<PyDescriptor> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let layers = self.0.layers()?;
        for layer in layers {
            if layer.digest() == digest {
                return Ok(layer);
            }
        }
        bail!("Layer {} not found", digest)
    }

    /// Get raw bytes of a blob by digest string or Descriptor.
    pub fn get_blob<'py>(
        &mut self,
        py: Python<'py>,
        digest_or_descriptor: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let digest: String = if let Ok(desc) = digest_or_descriptor.extract::<PyRef<PyDescriptor>>()
        {
            desc.digest()
        } else {
            digest_or_descriptor.extract::<String>()?
        };
        let blob = self
            .0
            .get_blob(&digest)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(PyBytes::new(py, &blob))
    }

    /// The first instance layer in the artifact.
    ///
    /// Raises `ValueError` if no instance layer is found.
    /// For multiple instance layers, use {meth}`get_instance` with a descriptor.
    #[getter(instance)]
    pub fn instance_(&mut self, py: Python<'_>) -> Result<crate::Instance> {
        Ok(self.get_instance(py, None)?)
    }

    /// The first solution layer in the artifact.
    ///
    /// Raises `ValueError` if no solution layer is found.
    /// For multiple solution layers, use {meth}`get_solution` with a descriptor.
    #[getter(solution)]
    pub fn solution_(&mut self, py: Python<'_>) -> Result<crate::Solution> {
        Ok(self.get_solution(py, None)?)
    }

    /// The first parametric instance layer in the artifact.
    ///
    /// Raises `ValueError` if no parametric instance layer is found.
    /// For multiple parametric instance layers, use {meth}`get_parametric_instance` with a descriptor.
    #[getter(parametric_instance)]
    pub fn parametric_instance_(&mut self, py: Python<'_>) -> Result<crate::ParametricInstance> {
        Ok(self.get_parametric_instance(py, None)?)
    }

    /// The first sample set layer in the artifact.
    ///
    /// Raises `ValueError` if no sample set layer is found.
    /// For multiple sample set layers, use {meth}`get_sample_set` with a descriptor.
    #[getter(sample_set)]
    pub fn sample_set_(&mut self, py: Python<'_>) -> Result<crate::SampleSet> {
        Ok(self.get_sample_set(py, None)?)
    }

    /// Get the layer object corresponding to the descriptor.
    ///
    /// Dynamically dispatched based on {attr}`~ommx.artifact.Descriptor.media_type`:
    /// - `application/org.ommx.v1.instance` returns {class}`~ommx.v1.Instance`
    /// - `application/org.ommx.v1.solution` returns {class}`~ommx.v1.Solution`
    /// - `application/vnd.numpy` returns a numpy array
    pub fn get_layer<'py>(
        &mut self,
        py: Python<'py>,
        descriptor: &PyDescriptor,
    ) -> PyResult<Bound<'py, PyAny>> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let media_type = descriptor.media_type();
        match media_type.as_str() {
            "application/org.ommx.v1.instance" => {
                let instance = self
                    .get_instance_inner(descriptor)
                    .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
                Ok(instance
                    .into_pyobject(py)?
                    .into_any()
                    .unbind()
                    .into_bound(py))
            }
            "application/org.ommx.v1.solution" => {
                let solution = self
                    .get_solution_inner(descriptor)
                    .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
                Ok(solution
                    .into_pyobject(py)?
                    .into_any()
                    .unbind()
                    .into_bound(py))
            }
            "application/vnd.numpy" => self.get_ndarray_inner(py, descriptor),
            _ => Err(pyo3::exceptions::PyValueError::new_err(format!(
                "Unsupported media type {}",
                media_type
            ))),
        }
    }

    /// Get an instance from the artifact.
    ///
    /// - If `descriptor` is `None`, returns the first instance layer.
    /// - If `descriptor` is given, returns the instance for that specific layer.
    ///
    /// Raises `ValueError` if no instance layer is found.
    #[pyo3(signature = (descriptor = None))]
    pub fn get_instance(
        &mut self,
        py: Python<'_>,
        descriptor: Option<&PyDescriptor>,
    ) -> PyResult<crate::Instance> {
        let _guard = crate::TRACING.attach_parent_context(py);
        match descriptor {
            Some(desc) => self
                .get_instance_inner(desc)
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string())),
            None => {
                let layers = self
                    .0
                    .layers()
                    .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
                for desc in &layers {
                    if desc.media_type() == "application/org.ommx.v1.instance" {
                        return self
                            .get_instance_inner(desc)
                            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()));
                    }
                }
                Err(pyo3::exceptions::PyValueError::new_err(
                    "Instance layer not found",
                ))
            }
        }
    }

    /// Get a solution from the artifact.
    ///
    /// - If `descriptor` is `None`, returns the first solution layer.
    /// - If `descriptor` is given, returns the solution for that specific layer.
    ///
    /// Raises `ValueError` if no solution layer is found.
    #[pyo3(signature = (descriptor = None))]
    pub fn get_solution(
        &mut self,
        py: Python<'_>,
        descriptor: Option<&PyDescriptor>,
    ) -> PyResult<crate::Solution> {
        let _guard = crate::TRACING.attach_parent_context(py);
        match descriptor {
            Some(desc) => self
                .get_solution_inner(desc)
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string())),
            None => {
                let layers = self
                    .0
                    .layers()
                    .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
                for desc in &layers {
                    if desc.media_type() == "application/org.ommx.v1.solution" {
                        return self
                            .get_solution_inner(desc)
                            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()));
                    }
                }
                Err(pyo3::exceptions::PyValueError::new_err(
                    "Solution layer not found",
                ))
            }
        }
    }

    /// Get a parametric instance from the artifact.
    ///
    /// - If `descriptor` is `None`, returns the first parametric instance layer.
    /// - If `descriptor` is given, returns the parametric instance for that specific layer.
    ///
    /// Raises `ValueError` if no parametric instance layer is found.
    #[pyo3(signature = (descriptor = None))]
    pub fn get_parametric_instance(
        &mut self,
        py: Python<'_>,
        descriptor: Option<&PyDescriptor>,
    ) -> PyResult<crate::ParametricInstance> {
        let _guard = crate::TRACING.attach_parent_context(py);
        match descriptor {
            Some(desc) => self
                .get_parametric_instance_inner(desc)
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string())),
            None => {
                let layers = self
                    .0
                    .layers()
                    .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
                for desc in &layers {
                    if desc.media_type() == "application/org.ommx.v1.parametric-instance" {
                        return self
                            .get_parametric_instance_inner(desc)
                            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()));
                    }
                }
                Err(pyo3::exceptions::PyValueError::new_err(
                    "Parametric instance layer not found",
                ))
            }
        }
    }

    /// Get a sample set from the artifact.
    ///
    /// - If `descriptor` is `None`, returns the first sample set layer.
    /// - If `descriptor` is given, returns the sample set for that specific layer.
    ///
    /// Raises `ValueError` if no sample set layer is found.
    #[pyo3(signature = (descriptor = None))]
    pub fn get_sample_set(
        &mut self,
        py: Python<'_>,
        descriptor: Option<&PyDescriptor>,
    ) -> PyResult<crate::SampleSet> {
        let _guard = crate::TRACING.attach_parent_context(py);
        match descriptor {
            Some(desc) => self
                .get_sample_set_inner(desc)
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string())),
            None => {
                let layers = self
                    .0
                    .layers()
                    .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
                for desc in &layers {
                    if desc.media_type() == "application/org.ommx.v1.sample-set" {
                        return self
                            .get_sample_set_inner(desc)
                            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()));
                    }
                }
                Err(pyo3::exceptions::PyValueError::new_err(
                    "Sample set layer not found",
                ))
            }
        }
    }

    /// Get a numpy array from an artifact layer stored by {meth}`~ommx.artifact.ArtifactBuilder.add_ndarray`.
    pub fn get_ndarray<'py>(
        &mut self,
        py: Python<'py>,
        descriptor: &PyDescriptor,
    ) -> PyResult<Bound<'py, PyAny>> {
        let _guard = crate::TRACING.attach_parent_context(py);
        self.get_ndarray_inner(py, descriptor)
    }

    /// Get a pandas DataFrame from an artifact layer stored by {meth}`~ommx.artifact.ArtifactBuilder.add_dataframe`.
    pub fn get_dataframe<'py>(
        &mut self,
        py: Python<'py>,
        descriptor: &PyDescriptor,
    ) -> PyResult<Bound<'py, PyAny>> {
        let _guard = crate::TRACING.attach_parent_context(py);
        assert_media_type(descriptor, "application/vnd.apache.parquet")?;
        let blob = self
            .0
            .get_blob(&descriptor.digest())
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        let io = py.import("io")?;
        let pandas = py.import("pandas")?;
        let bytes_io = io.call_method1("BytesIO", (PyBytes::new(py, &blob),))?;
        pandas.call_method1("read_parquet", (bytes_io,))
    }

    /// Get a JSON object from an artifact layer stored by {meth}`~ommx.artifact.ArtifactBuilder.add_json`.
    pub fn get_json<'py>(
        &mut self,
        py: Python<'py>,
        descriptor: &PyDescriptor,
    ) -> PyResult<Bound<'py, PyAny>> {
        let _guard = crate::TRACING.attach_parent_context(py);
        assert_media_type(descriptor, "application/json")?;
        let blob = self
            .0
            .get_blob(&descriptor.digest())
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        let json = py.import("json")?;
        json.call_method1("loads", (PyBytes::new(py, &blob),))
    }
}

impl PyArtifact {
    fn get_instance_inner(&mut self, descriptor: &PyDescriptor) -> Result<crate::Instance> {
        assert_media_type(descriptor, "application/org.ommx.v1.instance")?;
        let blob = self.0.get_blob(&descriptor.digest())?;
        Ok(crate::Instance {
            inner: ommx::Instance::from_bytes(&blob)?,
            annotations: descriptor.annotations(),
        })
    }

    fn get_solution_inner(&mut self, descriptor: &PyDescriptor) -> Result<crate::Solution> {
        assert_media_type(descriptor, "application/org.ommx.v1.solution")?;
        let blob = self.0.get_blob(&descriptor.digest())?;
        Ok(crate::Solution {
            inner: ommx::Solution::from_bytes(&blob)?,
            annotations: descriptor.annotations(),
        })
    }

    fn get_parametric_instance_inner(
        &mut self,
        descriptor: &PyDescriptor,
    ) -> Result<crate::ParametricInstance> {
        assert_media_type(descriptor, "application/org.ommx.v1.parametric-instance")?;
        let blob = self.0.get_blob(&descriptor.digest())?;
        Ok(crate::ParametricInstance {
            inner: ommx::ParametricInstance::from_bytes(&blob)?,
            annotations: descriptor.annotations(),
        })
    }

    fn get_sample_set_inner(&mut self, descriptor: &PyDescriptor) -> Result<crate::SampleSet> {
        assert_media_type(descriptor, "application/org.ommx.v1.sample-set")?;
        let blob = self.0.get_blob(&descriptor.digest())?;
        Ok(crate::SampleSet {
            inner: ommx::SampleSet::from_bytes(&blob)?,
            annotations: descriptor.annotations(),
        })
    }

    fn get_ndarray_inner<'py>(
        &mut self,
        py: Python<'py>,
        descriptor: &PyDescriptor,
    ) -> PyResult<Bound<'py, PyAny>> {
        assert_media_type(descriptor, "application/vnd.numpy")?;
        let blob = self
            .0
            .get_blob(&descriptor.digest())
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        let io = py.import("io")?;
        let numpy = py.import("numpy")?;
        let bytes_io = io.call_method1("BytesIO", (PyBytes::new(py, &blob),))?;
        numpy.call_method1("load", (bytes_io,))
    }
}

fn assert_media_type(descriptor: &PyDescriptor, expected: &str) -> Result<()> {
    let actual = descriptor.media_type();
    if actual != expected {
        bail!("Expected media type '{}', got '{}'", expected, actual);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// BuilderInner: unified wrapper for Archive / Dir builders
// ---------------------------------------------------------------------------

enum BuilderInner {
    Archive(Option<Box<ommx::artifact::Builder<ocipkg::image::OciArchiveBuilder>>>),
    Dir(Option<Box<ommx::artifact::LocalArtifactBuilder>>),
}

impl BuilderInner {
    fn add_layer(
        &mut self,
        media_type: &str,
        blob: &[u8],
        annotations: HashMap<String, String>,
    ) -> Result<PyDescriptor> {
        match self {
            BuilderInner::Archive(ref mut b) => {
                let builder = b
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("Already built artifact"))?;
                let desc = builder.add_layer(media_type.into(), blob, annotations)?;
                Ok(PyDescriptor::from(desc))
            }
            BuilderInner::Dir(ref mut b) => {
                let builder = b
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("Already built artifact"))?;
                let desc =
                    builder.add_layer_bytes(media_type.into(), blob.to_vec(), annotations)?;
                Ok(PyDescriptor::from(desc))
            }
        }
    }

    fn add_annotation(&mut self, key: &str, value: &str) -> Result<()> {
        match self {
            BuilderInner::Archive(ref mut b) => {
                let builder = b
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("Already built artifact"))?;
                builder.add_annotation(key.into(), value.into());
            }
            BuilderInner::Dir(ref mut b) => {
                let builder = b
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("Already built artifact"))?;
                builder.add_annotation(key, value);
            }
        }
        Ok(())
    }

    fn build(&mut self) -> Result<ArtifactInner> {
        match self {
            BuilderInner::Archive(ref mut b) => {
                let builder = b
                    .take()
                    .ok_or_else(|| anyhow::anyhow!("Already built artifact"))?;
                let artifact = (*builder).build()?;
                Ok(ArtifactInner::Archive(Mutex::new(artifact)))
            }
            BuilderInner::Dir(ref mut b) => {
                let builder = b
                    .take()
                    .ok_or_else(|| anyhow::anyhow!("Already built artifact"))?;
                let artifact = (*builder).build()?;
                Ok(ArtifactInner::Local(Box::new(Mutex::new(artifact))))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// PyArtifactBuilder
// ---------------------------------------------------------------------------

/// Builder for OMMX Artifacts.
///
/// ```python
/// >>> builder = ArtifactBuilder.temp()
/// >>> artifact = builder.build()
/// >>> print(artifact.image_name)
/// ttl.sh/...-...-...-...-...:1h
///
/// ```
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[pyo3(module = "ommx._ommx_rust", name = "ArtifactBuilder")]
pub struct PyArtifactBuilder(BuilderInner);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl PyArtifactBuilder {
    /// Create a new artifact archive with an unnamed image name.
    ///
    /// This cannot be loaded into local registry nor pushed to remote registry.
    ///
    /// ```python
    /// >>> from ommx.testing import SingleFeasibleLPGenerator, DataType
    /// >>> generator = SingleFeasibleLPGenerator(3, DataType.INT)
    /// >>> instance = generator.get_v1_instance()
    /// >>> import uuid
    /// >>> filename = f"data/single_feasible_lp.ommx.{uuid.uuid4()}"
    /// >>> builder = ArtifactBuilder.new_archive_unnamed(filename)
    /// >>> _desc = builder.add_instance(instance)
    /// >>> artifact = builder.build()
    /// >>> print(artifact.image_name)
    /// None
    ///
    /// ```
    #[staticmethod]
    pub fn new_archive_unnamed(path: PathBuf) -> Result<Self> {
        let builder = ommx::artifact::Builder::new_archive_unnamed(path)?;
        Ok(Self(BuilderInner::Archive(Some(Box::new(builder)))))
    }

    /// Create a new artifact archive with a named image name.
    #[staticmethod]
    pub fn new_archive(path: PathBuf, image_name: &str) -> Result<Self> {
        let image_name = ocipkg::ImageName::parse(image_name)?;
        let builder = ommx::artifact::Builder::new_archive(path, image_name)?;
        Ok(Self(BuilderInner::Archive(Some(Box::new(builder)))))
    }

    /// Create a new artifact in local registry with a named image name.
    ///
    /// ```python
    /// >>> from ommx.testing import SingleFeasibleLPGenerator, DataType
    /// >>> generator = SingleFeasibleLPGenerator(3, DataType.INT)
    /// >>> instance = generator.get_v1_instance()
    /// >>> import uuid
    /// >>> image_name = f"ghcr.io/jij-inc/ommx/single_feasible_lp:{uuid.uuid4()}"
    /// >>> builder = ArtifactBuilder.new(image_name)
    /// >>> _desc = builder.add_instance(instance)
    /// >>> artifact = builder.build()
    /// >>> print(artifact.image_name)
    /// ghcr.io/jij-inc/ommx/single_feasible_lp:...
    ///
    /// ```
    #[staticmethod]
    pub fn new(image_name: &str) -> Result<Self> {
        let image_name = ocipkg::ImageName::parse(image_name)?;
        let builder = ommx::artifact::LocalArtifactBuilder::new_ommx(image_name);
        Ok(Self(BuilderInner::Dir(Some(Box::new(builder)))))
    }

    /// Create a new artifact as a temporary file.
    ///
    /// Note that this is insecure and should only be used for testing.
    ///
    /// ```python
    /// >>> builder = ArtifactBuilder.temp()
    /// >>> artifact = builder.build()
    /// >>> print(artifact.image_name)
    /// ttl.sh/...-...-...-...-...:1h
    ///
    /// ```
    #[staticmethod]
    pub fn temp() -> Result<Self> {
        let builder = ommx::artifact::Builder::temp_archive()?;
        Ok(Self(BuilderInner::Archive(Some(Box::new(builder)))))
    }

    /// An alias for {meth}`new` to create a new artifact in local registry
    /// with GitHub Container Registry image name.
    ///
    /// This also sets the `org.opencontainers.image.source` annotation
    /// to the GitHub repository URL.
    #[staticmethod]
    pub fn for_github(org: &str, repo: &str, name: &str, tag: &str) -> Result<Self> {
        let builder = ommx::artifact::LocalArtifactBuilder::for_github(org, repo, name, tag)?;
        Ok(Self(BuilderInner::Dir(Some(Box::new(builder)))))
    }

    /// Add an {class}`~ommx.v1.Instance` to the artifact with annotations.
    ///
    /// ```python
    /// >>> from ommx.v1 import Instance
    /// >>> instance = Instance.empty()
    /// >>> instance.title = "test instance"
    /// >>> builder = ArtifactBuilder.temp()
    /// >>> desc = builder.add_instance(instance)
    /// >>> print(desc.annotations['org.ommx.v1.instance.title'])
    /// test instance
    ///
    /// ```
    pub fn add_instance(
        &mut self,
        py: Python<'_>,
        instance: &crate::Instance,
    ) -> Result<PyDescriptor> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let blob = instance.inner.to_bytes();
        self.0.add_layer(
            "application/org.ommx.v1.instance",
            &blob,
            instance.annotations.clone(),
        )
    }

    /// Add a {class}`~ommx.v1.ParametricInstance` to the artifact with annotations.
    pub fn add_parametric_instance(
        &mut self,
        py: Python<'_>,
        instance: &crate::ParametricInstance,
    ) -> Result<PyDescriptor> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let blob = instance.inner.to_bytes();
        self.0.add_layer(
            "application/org.ommx.v1.parametric-instance",
            &blob,
            instance.annotations.clone(),
        )
    }

    /// Add a {class}`~ommx.v1.Solution` to the artifact with annotations.
    pub fn add_solution(
        &mut self,
        py: Python<'_>,
        solution: &crate::Solution,
    ) -> Result<PyDescriptor> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let blob = solution.inner.to_bytes();
        self.0.add_layer(
            "application/org.ommx.v1.solution",
            &blob,
            solution.annotations.clone(),
        )
    }

    /// Add a {class}`~ommx.v1.SampleSet` to the artifact with annotations.
    pub fn add_sample_set(
        &mut self,
        py: Python<'_>,
        sample_set: &crate::SampleSet,
    ) -> Result<PyDescriptor> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let blob = sample_set.inner.to_bytes();
        self.0.add_layer(
            "application/org.ommx.v1.sample-set",
            &blob,
            sample_set.annotations.clone(),
        )
    }

    /// Add a numpy ndarray to the artifact with npy format.
    ///
    /// ```python
    /// >>> import numpy as np
    /// >>> array = np.array([1, 2, 3])
    /// >>> builder = ArtifactBuilder.temp()
    /// >>> _desc = builder.add_ndarray(array, title="test_array")
    /// >>> artifact = builder.build()
    /// >>> layer = artifact.layers[0]
    /// >>> print(layer.media_type)
    /// application/vnd.numpy
    /// >>> print(layer.annotations)
    /// {'org.ommx.user.title': 'test_array'}
    ///
    /// ```
    #[pyo3(signature = (array, *, annotation_namespace = "org.ommx.user.", **annotations))]
    pub fn add_ndarray(
        &mut self,
        py: Python,
        array: &Bound<PyAny>,
        annotation_namespace: &str,
        annotations: Option<&Bound<pyo3::types::PyDict>>,
    ) -> Result<PyDescriptor> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let io = py.import("io")?;
        let numpy = py.import("numpy")?;
        let bytes_io = io.call_method0("BytesIO")?;
        numpy.call_method1("save", (&bytes_io, array))?;
        let blob: Vec<u8> = bytes_io.call_method0("getvalue")?.extract()?;
        let ann = build_annotations(annotation_namespace, annotations)?;
        self.0.add_layer("application/vnd.numpy", &blob, ann)
    }

    /// Add a pandas DataFrame to the artifact with parquet format.
    ///
    /// ```python
    /// >>> import pandas as pd
    /// >>> df = pd.DataFrame({"a": [1, 2], "b": [3, 4]})
    /// >>> builder = ArtifactBuilder.temp()
    /// >>> _desc = builder.add_dataframe(df, title="test_dataframe")
    /// >>> artifact = builder.build()
    /// >>> layer = artifact.layers[0]
    /// >>> print(layer.media_type)
    /// application/vnd.apache.parquet
    ///
    /// ```
    #[pyo3(signature = (df, *, annotation_namespace = "org.ommx.user.", **annotations))]
    pub fn add_dataframe(
        &mut self,
        df: &Bound<PyAny>,
        annotation_namespace: &str,
        annotations: Option<&Bound<pyo3::types::PyDict>>,
    ) -> Result<PyDescriptor> {
        let _guard = crate::TRACING.attach_parent_context(df.py());
        let blob: Vec<u8> = df.call_method0("to_parquet")?.extract()?;
        let ann = build_annotations(annotation_namespace, annotations)?;
        self.0
            .add_layer("application/vnd.apache.parquet", &blob, ann)
    }

    /// Add a JSON object to the artifact.
    ///
    /// ```python
    /// >>> obj = {"a": 1, "b": 2}
    /// >>> builder = ArtifactBuilder.temp()
    /// >>> _desc = builder.add_json(obj, title="test_json")
    /// >>> artifact = builder.build()
    /// >>> layer = artifact.layers[0]
    /// >>> print(layer.media_type)
    /// application/json
    ///
    /// ```
    #[pyo3(signature = (obj, *, annotation_namespace = "org.ommx.user.", **annotations))]
    pub fn add_json(
        &mut self,
        py: Python,
        obj: &Bound<PyAny>,
        annotation_namespace: &str,
        annotations: Option<&Bound<pyo3::types::PyDict>>,
    ) -> Result<PyDescriptor> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let json = py.import("json")?;
        let blob_str: String = json.call_method1("dumps", (obj,))?.extract()?;
        let ann = build_annotations(annotation_namespace, annotations)?;
        self.0
            .add_layer("application/json", blob_str.as_bytes(), ann)
    }

    /// Low-level API to add any type of layer to the artifact with annotations.
    ///
    /// Use {meth}`add_instance` or other high-level methods if possible.
    #[pyo3(signature = (media_type, blob, annotations = HashMap::new()))]
    pub fn add_layer(
        &mut self,
        media_type: &str,
        blob: &Bound<PyBytes>,
        annotations: HashMap<String, String>,
    ) -> Result<PyDescriptor> {
        self.0.add_layer(media_type, blob.as_bytes(), annotations)
    }

    /// Add annotation to the artifact itself.
    pub fn add_annotation(&mut self, key: &str, value: &str) -> Result<()> {
        self.0.add_annotation(key, value)
    }

    /// Build the artifact.
    pub fn build(&mut self, py: Python<'_>) -> Result<PyArtifact> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let inner = self.0.build()?;
        Ok(PyArtifact(inner))
    }
}

/// Build annotation HashMap from namespace and **kwargs.
fn build_annotations(
    namespace: &str,
    annotations: Option<&Bound<pyo3::types::PyDict>>,
) -> Result<HashMap<String, String>> {
    let ns = if namespace.ends_with('.') {
        namespace.to_string()
    } else {
        format!("{namespace}.")
    };
    let mut result = HashMap::new();
    if let Some(dict) = annotations {
        for (key, value) in dict.iter() {
            let k: String = key.extract()?;
            let v: String = value.extract()?;
            result.insert(format!("{ns}{k}"), v);
        }
    }
    Ok(result)
}

// ---------------------------------------------------------------------------
// Module-level functions
// ---------------------------------------------------------------------------

/// Get the current OMMX Local Registry root path.
#[pyo3_stub_gen::derive::gen_stub_pyfunction]
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
#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
pub fn set_local_registry_root(path: PathBuf) -> Result<()> {
    ommx::artifact::set_local_registry_root(path)?;
    Ok(())
}

/// Get the path where given image is stored in the local registry.
///
/// - The directory may not exist if the image is not in the local registry.
///
#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
pub fn get_image_dir(image_name: &str) -> Result<PathBuf> {
    let image_name = ocipkg::ImageName::parse(image_name)?;
    Ok(ommx::artifact::get_image_dir(&image_name))
}

/// Get all image names stored in the local registry.
///
/// Returns a list of image names (as strings) found in the local registry.
///
#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
pub fn get_images() -> Result<Vec<String>> {
    let images = ommx::artifact::get_images()?;
    Ok(images.into_iter().map(|img| img.to_string()).collect())
}
