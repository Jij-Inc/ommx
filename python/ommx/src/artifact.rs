use anyhow::{bail, Result};
use pyo3::{prelude::*, types::PyBytes};
use std::{collections::HashMap, path::PathBuf};

use crate::PyDescriptor;

// ---------------------------------------------------------------------------
// PyArtifact backing handle
// ---------------------------------------------------------------------------
//
// v3 collapses the prior `Archive` / `Local` enum into a single
// `LocalArtifact`: archives are an exchange format that must be
// imported into the SQLite Local Registry before any read / push
// happens, so every PyArtifact value points into the user's
// persistent SQLite registry.

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
pub struct PyArtifact(ommx::artifact::LocalArtifact);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl PyArtifact {
    /// Import an artifact from a `.ommx` OCI archive file (or an OCI
    /// Image Layout directory) into the user's v3 SQLite Local Registry,
    /// and return a handle to the imported registry entry.
    ///
    /// **Side effect (intentional)**: archive / directory contents are
    /// permanently written into the SQLite Local Registry under the
    /// default root (`$XDG_DATA_HOME/ommx/` on Linux,
    /// `$HOME/Library/Application Support/org.ommx.ommx/` on macOS, or
    /// `$OMMX_LOCAL_REGISTRY_ROOT` when set). Subsequent
    /// `Artifact.load(image_name)` calls resolve from SQLite without
    /// re-importing. v3 treats `.ommx` files purely as an exchange
    /// format; there is no in-place "open archive for read" path.
    ///
    /// The input must carry an `org.opencontainers.image.ref.name`
    /// annotation. Unnamed archives / directories cannot be addressed
    /// in the SQLite Local Registry and are rejected.
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
        let registry =
            std::sync::Arc::new(ommx::artifact::local_registry::LocalRegistry::open_default()?);

        let image_name = if path.is_file() {
            // Archive ingest goes through `import_oci_archive` which
            // streams blobs into the user's FileBlobStore and publishes
            // the manifest atomically. Conflicts on KeepExisting are
            // surfaced as `Err` by the import path itself.
            let outcome = ommx::artifact::local_registry::import_oci_archive(&registry, &path)?;
            outcome.image_name.ok_or_else(|| {
                anyhow::anyhow!(
                    "OCI archive at {} has no `org.opencontainers.image.ref.name` \
                     annotation; v3 SQLite Local Registry requires a ref name to \
                     address an imported artifact. Rebuild the archive with an \
                     image name via `ArtifactBuilder.new(image_name)` + `Artifact.save(path)`.",
                    path.display(),
                )
            })?
        } else if path.is_dir() {
            // Validate the ref annotation BEFORE running `import_oci_dir`
            // so an unnamed OCI Image Layout (no
            // `org.opencontainers.image.ref.name`) bails without
            // mutating the user's SQLite registry. Otherwise blobs +
            // manifest are persisted under no ref and the same call
            // re-orphans them on every retry.
            let image_name = ommx::artifact::local_registry::oci_dir_image_name(&path)?
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "OCI dir at {} has no `org.opencontainers.image.ref.name` \
                         annotation; unannotated OCI Image Layouts cannot be addressed \
                         by name in the SQLite Local Registry",
                        path.display(),
                    )
                })?;
            ommx::artifact::local_registry::import_oci_dir(
                registry.index(),
                registry.blobs(),
                &path,
            )?;
            image_name
        } else {
            bail!("Path must be a file or a directory")
        };

        let artifact = ommx::artifact::LocalArtifact::open_in_registry(registry, image_name)?;
        Ok(Self(artifact))
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

        // Fast path: the image is already published in the v3 SQLite Local
        // Registry. Subsequent calls for the same image always land here.
        if let Some(artifact) = ommx::artifact::LocalArtifact::try_open(image_name_parsed.clone())?
        {
            return Ok(Self(artifact));
        }

        // SQLite miss — pull from the remote registry directly into
        // SQLite via the v3 native `pull_image` (no on-disk OCI dir
        // intermediate; blobs land straight in FileBlobStore).
        let registry =
            std::sync::Arc::new(ommx::artifact::local_registry::LocalRegistry::open_default()?);
        ommx::artifact::local_registry::pull_image(&registry, &image_name_parsed)?;
        let artifact =
            ommx::artifact::LocalArtifact::open_in_registry(registry, image_name_parsed)?;
        Ok(Self(artifact))
    }

    /// Push the artifact to remote registry.
    #[cfg(feature = "remote-artifact")]
    pub fn push(&mut self, py: Python<'_>) -> Result<()> {
        let _guard = crate::TRACING.attach_parent_context(py);
        self.0.push()
    }

    /// Save the artifact as a `.ommx` OCI archive file at `path`.
    ///
    /// The archive is an exchange-format export of the registry-resident
    /// artifact. Loading the archive back via
    /// {meth}`Artifact.load_archive` reimports it into the SQLite Local
    /// Registry under the same image name.
    pub fn save(&mut self, py: Python<'_>, path: PathBuf) -> Result<()> {
        let _guard = crate::TRACING.attach_parent_context(py);
        self.0.save(&path)
    }

    #[getter]
    pub fn image_name(&mut self) -> Option<String> {
        Some(self.0.image_name().to_string())
    }

    /// Annotations in the artifact manifest.
    #[getter]
    pub fn annotations(&mut self) -> Result<HashMap<String, String>> {
        self.0.annotations()
    }

    #[getter]
    pub fn layers(&mut self) -> Result<Vec<PyDescriptor>> {
        Ok(self
            .0
            .layers()?
            .into_iter()
            .map(PyDescriptor::from)
            .collect())
    }

    /// Look up a layer descriptor by digest.
    pub fn get_layer_descriptor(&mut self, py: Python<'_>, digest: &str) -> Result<PyDescriptor> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let layers = self.0.layers()?;
        for layer in layers {
            if layer.digest().as_ref() == digest {
                return Ok(PyDescriptor::from(layer));
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
                for desc in layers {
                    let py_desc = PyDescriptor::from(desc);
                    if py_desc.media_type() == "application/org.ommx.v1.instance" {
                        return self
                            .get_instance_inner(&py_desc)
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
                for desc in layers {
                    let py_desc = PyDescriptor::from(desc);
                    if py_desc.media_type() == "application/org.ommx.v1.solution" {
                        return self
                            .get_solution_inner(&py_desc)
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
                for desc in layers {
                    let py_desc = PyDescriptor::from(desc);
                    if py_desc.media_type() == "application/org.ommx.v1.parametric-instance" {
                        return self
                            .get_parametric_instance_inner(&py_desc)
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
                for desc in layers {
                    let py_desc = PyDescriptor::from(desc);
                    if py_desc.media_type() == "application/org.ommx.v1.sample-set" {
                        return self
                            .get_sample_set_inner(&py_desc)
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
// BuilderInner: wrapper around LocalArtifactBuilder
// ---------------------------------------------------------------------------
//
// v3 collapses the old `Archive` / `Local` split: archives are an
// exchange format produced by `LocalArtifact::save(path)`, not a
// distinct build target. Every builder lands in the user's SQLite
// Local Registry; callers `save()` afterward if they also want a
// `.ommx` file. The `Option` is consumed on `build()` so a second
// call surfaces "Already built artifact".

struct BuilderInner(Option<Box<ommx::artifact::LocalArtifactBuilder>>);

impl BuilderInner {
    fn new(builder: ommx::artifact::LocalArtifactBuilder) -> Self {
        Self(Some(Box::new(builder)))
    }

    fn as_mut(&mut self) -> Result<&mut ommx::artifact::LocalArtifactBuilder> {
        self.0
            .as_mut()
            .map(|b| b.as_mut())
            .ok_or_else(|| anyhow::anyhow!("Already built artifact"))
    }

    fn add_layer(
        &mut self,
        media_type: &str,
        blob: &[u8],
        annotations: HashMap<String, String>,
    ) -> Result<PyDescriptor> {
        let desc = self
            .as_mut()?
            .add_layer_bytes(media_type.into(), blob.to_vec(), annotations)?;
        Ok(PyDescriptor::from(desc))
    }

    fn add_annotation(&mut self, key: &str, value: &str) -> Result<()> {
        self.as_mut()?.add_annotation(key, value);
        Ok(())
    }

    fn build(&mut self) -> Result<ommx::artifact::LocalArtifact> {
        let builder = self
            .0
            .take()
            .ok_or_else(|| anyhow::anyhow!("Already built artifact"))?;
        (*builder).build()
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
    /// Create a new artifact builder with an explicit image name. The
    /// artifact is published into the user's persistent SQLite Local
    /// Registry on `build()`; call {meth}`Artifact.save(path)` on the
    /// returned handle if you also want a `.ommx` archive file for
    /// sharing.
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
        let builder = ommx::artifact::LocalArtifactBuilder::new(image_name);
        Ok(Self(BuilderInner::new(builder)))
    }

    /// Create a new artifact builder without inventing an image name.
    ///
    /// UX shortcut: a synthetic image name of the form
    /// `<registry-id8>.ommx.local/anonymous:<local-timestamp>` is
    /// generated at build time and used as the SQLite Local Registry
    /// key. v3 stores every artifact in the registry, so anonymous
    /// artifacts still need a key — the registry-id prefix (a random
    /// 8-hex truncation of a UUID generated once per `LocalRegistry`
    /// and persisted in its SQLite metadata) identifies which
    /// registry produced the artifact, useful when archives are
    /// shared, and the local-time timestamp lets you identify entries
    /// by when they were created. Use `Artifact.image_name` to read
    /// the synthesized name back. The `.local` mDNS TLD prevents an
    /// accidental push from leaking to a real remote registry. Use
    /// `ommx artifact prune-anonymous` to clean accumulated entries.
    ///
    /// Call {meth}`Artifact.save(path)` on the returned handle to also
    /// write a `.ommx` archive file for sharing.
    ///
    /// ```python
    /// >>> from ommx.testing import SingleFeasibleLPGenerator, DataType
    /// >>> generator = SingleFeasibleLPGenerator(3, DataType.INT)
    /// >>> instance = generator.get_v1_instance()
    /// >>> builder = ArtifactBuilder.new_anonymous()
    /// >>> _desc = builder.add_instance(instance)
    /// >>> artifact = builder.build()
    /// >>> assert ".ommx.local/anonymous:" in artifact.image_name
    ///
    /// ```
    #[staticmethod]
    pub fn new_anonymous() -> Result<Self> {
        let builder = ommx::artifact::LocalArtifactBuilder::new_anonymous();
        Ok(Self(BuilderInner::new(builder)))
    }

    /// Create a new artifact builder under a random `ttl.sh` image name.
    /// Insecure; for tests only. `ttl.sh` is a public registry that
    /// expires images after one hour.
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
        let builder = ommx::artifact::LocalArtifactBuilder::temp()?;
        Ok(Self(BuilderInner::new(builder)))
    }

    /// An alias for {meth}`new` to create a new artifact in local registry
    /// with GitHub Container Registry image name.
    ///
    /// This also sets the `org.opencontainers.image.source` annotation
    /// to the GitHub repository URL.
    #[staticmethod]
    pub fn for_github(org: &str, repo: &str, name: &str, tag: &str) -> Result<Self> {
        let builder = ommx::artifact::LocalArtifactBuilder::for_github(org, repo, name, tag)?;
        Ok(Self(BuilderInner::new(builder)))
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
