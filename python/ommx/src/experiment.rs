use anyhow::{Context, Result};
use oci_spec::image::MediaType;
use pyo3::{
    exceptions::PyKeyboardInterrupt,
    prelude::*,
    types::{PyBool, PyBytes, PyDict, PyFloat, PyInt, PyList, PyString, PyType, PyTypeMethods},
};
use std::{
    collections::{btree_map::Entry, BTreeMap, HashMap},
    mem,
    path::PathBuf,
};

use crate::pandas::{raw_entries_to_dataframe, PyDataFrame};
use crate::PyArtifact;
use ommx::artifact::{media_types, AsArtifact};
use ommx::experiment::{AttachmentLogger, SolveDiagnosticPayload};

#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[pyo3(module = "ommx._ommx_rust", name = "Experiment")]
/// A collection of optimization experiment records stored as one OMMX Artifact.
///
/// An `Experiment` owns experiment-level attachments and a sequence of
/// closed `Run` records. Each `Run` can store scalar run parameters,
/// run-level attachments, and zero or more `Solve` records.
///
/// Newly created experiments are unsealed. Call `commit()` to write the
/// experiment into the local registry as an OMMX Artifact. After commit, the
/// same object can be used as a read-only view of the committed artifact.
/// `with Experiment(...)` commits on normal exit if the experiment is still
/// unsealed. On exception it does not advance the successful Experiment image
/// reference; instead it tries to publish a local checkpoint with status
/// `"failed"` or `"interrupted"`.
///
/// Logging APIs store payload bytes in the Local Registry immediately. The
/// final commit writes an Experiment config and manifest that make those
/// payloads reachable as one immutable Artifact. A closed `Run` also publishes
/// a best-effort `"draft"` checkpoint so a later process can resume from the
/// latest closed Run with `Experiment.restore_from_checkpoint(...)`.
///
/// Use experiment-level attachments for shared context such as dataset or
/// source-problem metadata. Use `Run.log_parameter(...)` for scalar values
/// that should appear in `run_parameters_df()`, and use run attachments or
/// `Run.log_solve(...)` for payloads that belong to a specific run.
///
/// Example:
///
/// >>> from ommx.experiment import Experiment
/// >>> with Experiment.with_temp_local_registry() as exp:
/// ...     exp.log_json("dataset", {"name": "demo"})
/// ...     with exp.run() as run:
/// ...         run.log_parameter("capacity", 10)
/// ...         run.log_json("scenario", {"capacity": 10})
/// >>> len(exp.runs)
/// 1
/// >>> len(exp.attachment_names)
/// 1
/// >>> exp.run_parameters_df().to_dict()
/// {'capacity': {0: 10}}
///
/// If the experiment has only one run, open the experiment and the run in
/// one `with` statement. On normal exit, the run is finished first and then
/// the experiment is committed:
///
/// >>> with Experiment() as exp, exp.run() as run:  # doctest: +SKIP
/// ...     solution = run.log_solve(adapter, instance, time_limit=10.0)
/// >>> exp.rename("ghcr.io/container/name:latest")  # doctest: +SKIP
/// >>> exp.push()  # doctest: +SKIP
pub struct PyExperiment {
    inner: ommx::experiment::ExperimentDyn,
    store_trace: bool,
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl PyExperiment {
    /// Start a new Experiment in the default local registry.
    ///
    /// If `image_name` is omitted, OMMX generates an anonymous local
    /// Experiment name. Pass an OCI image reference such as
    /// `"example.com/team/experiment:tag"` when the experiment should be
    /// loaded later by name. The image reference is a mutable local registry
    /// alias for the committed Artifact; the Artifact manifest digest remains
    /// the immutable identity of the committed contents.
    ///
    /// Set `store_trace=True` to store traces for `Run` context managers
    /// created from this Experiment. The Experiment itself does not need to
    /// be used as a context manager.
    #[new]
    #[pyo3(signature = (image_name = None, *, store_trace = false))]
    pub fn new(image_name: Option<&str>, store_trace: bool) -> Result<Self> {
        Ok(Self {
            inner: ommx::experiment::ExperimentDyn::new(parse_name(image_name)?)?,
            store_trace,
        })
    }

    /// Start a new Experiment backed by a temporary Local Registry.
    ///
    /// The temporary registry is kept alive by the returned Experiment
    /// and by Artifacts / loaded Experiments derived from it. This is useful
    /// for examples and tests because it does not write entries into the
    /// process-wide default local registry.
    ///
    /// Set `store_trace=True` to store traces for `Run` context managers
    /// created from this Experiment. The Experiment itself does not need to
    /// be used as a context manager.
    #[staticmethod]
    #[pyo3(signature = (image_name = None, *, store_trace = false))]
    pub fn with_temp_local_registry(image_name: Option<&str>, store_trace: bool) -> Result<Self> {
        Ok(Self {
            inner: ommx::experiment::ExperimentDyn::with_temp_local_registry(parse_name(
                image_name,
            )?)?,
            store_trace,
        })
    }

    /// Load a committed Experiment Artifact by image reference.
    ///
    /// If the image is not found in the default Local Registry, OMMX tries to
    /// pull it from the remote registry, matching {meth}`Artifact.load`.
    /// The loaded artifact must contain an Experiment config. Use
    /// `Experiment(...)` to create a new unsealed experiment.
    #[staticmethod]
    pub fn load(py: Python<'_>, image_name: &str) -> Result<Self> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let image_name = ommx::artifact::ImageRef::parse(image_name)?;
        Ok(Self {
            inner: ommx::experiment::ExperimentDyn::load(image_name)?,
            store_trace: false,
        })
    }

    /// Restore an unsealed Experiment from its checkpoint.
    ///
    /// Pass the original requested Experiment image name, not the generated
    /// checkpoint ref. This accepts checkpoint statuses such as `draft`,
    /// `failed`, or `interrupted`, and returns a new unsealed Experiment whose
    /// image name is the original requested Experiment image name recorded in
    /// the checkpoint metadata. Checkpoint Artifact handles and checkpoint
    /// image names are not part of the public API.
    #[staticmethod]
    pub fn restore_from_checkpoint(py: Python<'_>, image_name: &str) -> Result<Self> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let image_name = ommx::artifact::ImageRef::parse(image_name)?;
        Ok(Self {
            inner: ommx::experiment::ExperimentDyn::restore_from_checkpoint(image_name)?,
            store_trace: false,
        })
    }

    /// Import an Experiment Artifact from a `.ommx` OCI archive file (or an OCI
    /// Image Layout directory).
    ///
    /// The archive is imported into the default Local Registry, matching
    /// {meth}`Artifact.import_archive`, and then interpreted as an
    /// Experiment. The imported artifact must contain an Experiment config.
    #[staticmethod]
    pub fn import_archive(py: Python<'_>, path: PathBuf) -> Result<Self> {
        let _guard = crate::TRACING.attach_parent_context(py);
        Ok(Self {
            inner: ommx::experiment::ExperimentDyn::import_archive(&path)?,
            store_trace: false,
        })
    }

    /// Interpret an already-open Artifact as a committed Experiment.
    ///
    /// This is the usual entry point after importing or receiving an OMMX
    /// Artifact handle. The artifact must contain an Experiment config.
    #[staticmethod]
    pub fn from_artifact(artifact: &PyArtifact) -> Result<Self> {
        Ok(Self {
            inner: ommx::experiment::ExperimentDyn::from_artifact(artifact.inner().clone())?,
            store_trace: false,
        })
    }

    /// Fork this committed Experiment into a new unsealed child Experiment.
    ///
    /// The parent Experiment is not modified. Existing Attachments, Runs,
    /// Solves, and Run parameters are carried into the child Experiment.
    /// When the child is committed, its Artifact manifest records the parent
    /// manifest descriptor as OCI `subject`. The child reuses payload blobs
    /// already present in the Local Registry; forking creates a new manifest
    /// but does not duplicate unchanged Instance, Solution, or Attachment
    /// bytes.
    ///
    /// If `image_name` is omitted, OMMX generates an anonymous local
    /// Experiment name for the child. The returned Experiment can be used as
    /// a context manager:
    ///
    /// ```python
    /// with parent.fork() as child:
    ///     with child.run() as run:
    ///         run.log_parameter("capacity", 56)
    /// ```
    ///
    /// Raises an error if this Experiment has not been committed yet.
    ///
    /// Set `store_trace=True` on the returned child to store traces for
    /// `Run` context managers created from it. The child Experiment itself
    /// does not need to be used as a context manager.
    #[pyo3(signature = (image_name = None, *, store_trace = false))]
    pub fn fork(&self, image_name: Option<&str>, store_trace: bool) -> Result<Self> {
        Ok(Self {
            inner: self.inner.fork(parse_name(image_name)?)?,
            store_trace,
        })
    }

    pub fn __enter__(slf: Bound<'_, Self>) -> PyResult<Py<PyExperiment>> {
        Ok(slf.unbind())
    }

    #[pyo3(signature = (exc_type = None, exc_value = None, traceback = None))]
    pub fn __exit__(
        &mut self,
        py: Python<'_>,
        exc_type: Option<&Bound<'_, PyAny>>,
        exc_value: Option<&Bound<'_, PyAny>>,
        traceback: Option<&Bound<'_, PyAny>>,
    ) -> Result<bool> {
        if exc_type.is_some() && self.inner.is_unsealed() {
            let reason = python_exception_reason(exc_type, exc_value);
            let checkpoint = if is_keyboard_interrupt(py, exc_type)? {
                self.inner.commit_interrupted_checkpoint(reason)
            } else {
                self.inner.commit_failed_checkpoint(reason)
            };
            if let Err(error) = checkpoint {
                tracing::warn!(
                    error = %error,
                    "Failed to publish Experiment checkpoint during exception exit"
                );
            }
        } else if self.inner.is_unsealed() {
            self.commit_inner(py)?;
        }
        let _ = traceback;
        Ok(false)
    }

    #[getter]
    /// OCI image reference used to store this Experiment in a local registry.
    pub fn image_name(&self) -> Result<String> {
        Ok(self.inner.image_name()?.to_string())
    }

    /// Rename this Experiment to another local registry image reference.
    ///
    /// Before commit, this changes the image reference that `commit()` will
    /// publish. After commit, it publishes the same Artifact manifest under
    /// `image_name` and updates this handle to use the new name. The previous
    /// name remains as an alias in the Local Registry.
    pub fn rename(&mut self, image_name: &str) -> Result<()> {
        let image_name = ommx::artifact::ImageRef::parse(image_name)?;
        self.inner.rename(image_name)
    }

    /// Save this committed Experiment Artifact as a `.ommx` OCI archive file at `path`.
    ///
    /// The archive is an exchange-format export of the registry-resident
    /// Experiment Artifact. Loading the archive back via
    /// {meth}`Experiment.import_archive` reimports it into the SQLite Local
    /// Registry under the same image name.
    ///
    /// Raises an error if the Experiment has not been committed yet.
    pub fn save(&mut self, py: Python<'_>, path: PathBuf) -> Result<()> {
        let _guard = crate::TRACING.attach_parent_context(py);
        self.inner.save(&path)
    }

    /// Push this committed Experiment Artifact to its remote registry.
    ///
    /// Use `rename(...)` first when an anonymous or local-only experiment
    /// should be published under a remote container image reference.
    ///
    /// Raises an error if the Experiment has not been committed yet.
    #[cfg(feature = "remote-artifact")]
    pub fn push(&mut self, py: Python<'_>) -> Result<()> {
        let _guard = crate::TRACING.attach_parent_context(py);
        self.inner.push()
    }

    #[getter]
    /// Names of experiment-level attachments.
    pub fn attachment_names(&self) -> Result<Vec<String>> {
        self.inner.attachment_names()
    }

    /// OCI media type of an experiment-level attachment.
    pub fn attachment_media_type(&self, name: &str) -> Result<String> {
        Ok(self.inner.attachment_media_type(name)?.to_string())
    }

    /// Read an experiment-level attachment by name.
    ///
    /// The returned Python object is decoded from the attachment media type:
    /// JSON attachments become normal Python objects, OMMX instance-like
    /// attachments become the corresponding `ommx.v1` objects, and unknown
    /// media types are returned as raw `bytes`.
    pub fn get_attachment<'py>(&self, py: Python<'py>, name: &str) -> Result<Bound<'py, PyAny>> {
        let _guard = crate::TRACING.attach_parent_context(py);
        decode_experiment_attachment(py, &self.inner, name)
    }

    /// Read a JSON experiment-level attachment by name.
    ///
    /// Raises an error if the attachment exists but its media type is not
    /// `application/json`.
    pub fn get_json<'py>(&self, py: Python<'py>, name: &str) -> Result<Bound<'py, PyAny>> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let expected = MediaType::from("application/json");
        ensure_media_type(&self.inner.attachment_media_type(name)?, &expected)?;
        decode_json_blob(py, &self.inner.attachment_blob(name)?)
    }

    /// Read an Instance experiment-level attachment by name.
    pub fn get_instance(&self, py: Python<'_>, name: &str) -> Result<crate::Instance> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let (inner, annotations) = self.inner.attachment_instance(name)?;
        Ok(crate::Instance { inner, annotations })
    }

    /// Read a ParametricInstance experiment-level attachment by name.
    pub fn get_parametric_instance(
        &self,
        py: Python<'_>,
        name: &str,
    ) -> Result<crate::ParametricInstance> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let (inner, annotations) = self.inner.attachment_parametric_instance(name)?;
        Ok(crate::ParametricInstance { inner, annotations })
    }

    /// Read a Solution experiment-level attachment by name.
    pub fn get_solution(&self, py: Python<'_>, name: &str) -> Result<crate::Solution> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let (inner, annotations) = self.inner.attachment_solution(name)?;
        Ok(crate::Solution { inner, annotations })
    }

    /// Read a SampleSet experiment-level attachment by name.
    pub fn get_sample_set(&self, py: Python<'_>, name: &str) -> Result<crate::SampleSet> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let (inner, annotations) = self.inner.attachment_sample_set(name)?;
        Ok(crate::SampleSet { inner, annotations })
    }

    /// Read raw bytes of an experiment-level attachment by name.
    pub fn get_blob<'py>(&self, py: Python<'py>, name: &str) -> Result<Bound<'py, PyBytes>> {
        let _guard = crate::TRACING.attach_parent_context(py);
        Ok(PyBytes::new(py, &self.inner.attachment_blob(name)?))
    }

    /// Write an experiment-level attachment to a filesystem path.
    ///
    /// If `path` names an existing directory, the attachment filename stored
    /// by `log_file` is used inside that directory. Otherwise `path` is
    /// treated as the destination file path.
    #[pyo3(signature = (name, path, *, overwrite = false))]
    pub fn write_attachment(
        &self,
        py: Python<'_>,
        name: &str,
        path: PathBuf,
        overwrite: bool,
    ) -> Result<PathBuf> {
        let _guard = crate::TRACING.attach_parent_context(py);
        self.inner.write_attachment(name, path, overwrite)
    }

    /// Read an experiment-level attachment by name and decode it with a codec.
    ///
    /// The codec class must provide `media_type`, `encode(value) -> bytes`,
    /// and `decode(bytes) -> object`. This method validates the stored media
    /// type against the codec before decoding.
    pub fn get_with_codec(
        &self,
        py: Python<'_>,
        codec: AttachmentCodecInput,
        name: &str,
    ) -> Result<AttachmentPayload> {
        let _guard = crate::TRACING.attach_parent_context(py);
        codec.decode_from_parts(
            py,
            self.inner.attachment_media_type(name)?,
            &self.inner.attachment_blob(name)?,
        )
    }

    #[getter]
    /// Closed runs in insertion order.
    pub fn runs(&self) -> Result<Vec<PySealedRun>> {
        let runs = self.inner.runs()?;
        Ok(runs.into_iter().map(|run| PySealedRun { run }).collect())
    }

    #[getter]
    /// Experiment config status for a committed Experiment.
    ///
    /// Returns `None` for an unsealed Experiment.
    pub fn status(&self) -> Option<String> {
        self.inner
            .experiment_status()
            .map(|status| status.to_string())
    }

    #[getter]
    /// Committed OMMX Artifact for this Experiment.
    ///
    /// Raises an error if the Experiment has not been committed yet.
    pub fn artifact(&self) -> Result<PyArtifact> {
        Ok(PyArtifact::new(self.inner.artifact()?))
    }

    /// Start a new Run in this unsealed Experiment.
    ///
    /// The returned `Run` must be closed before `commit()`. Use it as a
    /// context manager to close it automatically on normal or exceptional exit:
    ///
    /// ```python
    /// with experiment.run() as run:
    ///     run.log_parameter("capacity", 47)
    /// ```
    ///
    /// Closing a Run records its status as `"finished"`, `"failed"`, or
    /// `"interrupted"` and publishes a best-effort draft checkpoint for the
    /// parent Experiment. Payloads written by an open Run before it is closed
    /// are stored in the Local Registry but are not recoverable through a
    /// checkpoint until the Run is closed.
    pub fn run(&self) -> Result<PyRun> {
        Ok(PyRun {
            state: PyRunState::Open {
                run: self.inner.run()?,
            },
            store_trace: self.store_trace,
        })
    }

    /// Attach arbitrary bytes with an explicit OCI media type in the experiment space.
    ///
    /// The `name` is stored as attachment metadata and is intended for
    /// humans. The bytes are stored as a layer in the committed artifact.
    pub fn log_attachment(
        &mut self,
        name: &str,
        media_type: &str,
        bytes: &Bound<pyo3::types::PyBytes>,
    ) -> Result<()> {
        AttachmentLogger::log_attachment(
            &self.inner,
            name,
            MediaType::from(media_type),
            bytes.as_bytes(),
            HashMap::new(),
        )
    }

    /// Attach an existing filesystem file in the experiment space.
    ///
    /// The file bytes are copied into the Local Registry immediately. If
    /// `media_type` is omitted, the Rust SDK infers it from file contents and
    /// unknown types fall back to `application/octet-stream`. The original
    /// source path is not stored; only a basename for later export is stored
    /// as attachment metadata.
    #[pyo3(signature = (name, path, media_type = None, *, filename = None))]
    pub fn log_file(
        &mut self,
        py: Python<'_>,
        name: &str,
        path: PathBuf,
        media_type: Option<&str>,
        filename: Option<&str>,
    ) -> Result<()> {
        let _guard = crate::TRACING.attach_parent_context(py);
        AttachmentLogger::log_file(
            &self.inner,
            name,
            &path,
            media_type.map(MediaType::from),
            filename,
        )
    }

    /// Encode a Python object with an attachment codec and attach it in the experiment space.
    ///
    /// The codec class must provide `media_type`, `encode(value) -> bytes`,
    /// and `decode(bytes) -> object`. OMMX owns only this protocol; concrete
    /// codecs should live in the package that owns the payload type.
    pub fn log_with_codec(
        &mut self,
        py: Python<'_>,
        codec: AttachmentCodecInput,
        name: &str,
        value: AttachmentPayload,
    ) -> Result<()> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let attachment = codec.encode(py, &value)?;
        AttachmentLogger::log_attachment(
            &self.inner,
            name,
            attachment.media_type,
            attachment.bytes,
            HashMap::new(),
        )
    }

    /// Attach a JSON-serializable value in the experiment space.
    ///
    /// The value is encoded with Python's `json.dumps` and stored with media
    /// type `application/json`.
    pub fn log_json(&mut self, py: Python<'_>, name: &str, value: &Bound<PyAny>) -> Result<()> {
        let json = py.import("json")?;
        let blob: String = json.call_method1("dumps", (value,))?.extract()?;
        AttachmentLogger::log_attachment(
            &self.inner,
            name,
            MediaType::from("application/json"),
            blob,
            HashMap::new(),
        )
    }

    /// Attach an Instance in the experiment space.
    pub fn log_instance(&mut self, name: &str, instance: &crate::Instance) -> Result<()> {
        AttachmentLogger::log_instance(
            &self.inner,
            name,
            &instance.inner,
            instance.annotations.clone(),
        )
    }

    /// Attach an ParametricInstance in the experiment space.
    pub fn log_parametric_instance(
        &mut self,
        name: &str,
        pi: &crate::ParametricInstance,
    ) -> Result<()> {
        AttachmentLogger::log_parametric_instance(
            &self.inner,
            name,
            &pi.inner,
            pi.annotations.clone(),
        )
    }

    /// Attach a Solution in the experiment space.
    pub fn log_solution(&mut self, name: &str, solution: &crate::Solution) -> Result<()> {
        AttachmentLogger::log_solution(
            &self.inner,
            name,
            &solution.inner,
            solution.annotations.clone(),
        )
    }

    /// Attach a SampleSet in the experiment space.
    pub fn log_sample_set(&mut self, name: &str, sample_set: &crate::SampleSet) -> Result<()> {
        AttachmentLogger::log_sample_set(
            &self.inner,
            name,
            &sample_set.inner,
            sample_set.annotations.clone(),
        )
    }

    /// Commit this unsealed Experiment into the local registry.
    ///
    /// All open runs must be closed before committing. The returned
    /// `Artifact` can be saved as a `.ommx` archive or passed to
    /// `Experiment.from_artifact`. After commit, this object becomes a
    /// read-only view of the committed Experiment. A successful commit
    /// publishes the requested image reference and removes any local checkpoint
    /// for that Experiment when present.
    pub fn commit(&mut self, py: Python<'_>) -> Result<PyArtifact> {
        self.commit_inner(py)
    }

    /// Wide DataFrame of run parameters, indexed by `run_id`.
    ///
    /// Run parameters are scalar values logged with `Run.log_parameter`.
    /// Closed runs with no parameters are still present as index rows.
    /// Adapter options recorded by `Run.log_solve` are solve metadata and do
    /// not appear in this table.
    pub fn run_parameters_df<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyDataFrame>> {
        let mut rows = BTreeMap::new();
        for run in self.inner.runs()? {
            let run_id = run.run_id();
            let dict = PyDict::new(py);
            dict.set_item("run_id", run_id)?;
            rows.insert(run_id, dict);
        }
        for cell in self.inner.run_parameter_cells()? {
            let row = match rows.entry(cell.run_id) {
                Entry::Occupied(entry) => entry.into_mut(),
                Entry::Vacant(entry) => {
                    let dict = PyDict::new(py);
                    dict.set_item("run_id", cell.run_id)?;
                    entry.insert(dict)
                }
            };
            match cell.value {
                ommx::experiment::ParameterValue::Bool(value) => {
                    row.set_item(cell.name, value)?;
                }
                ommx::experiment::ParameterValue::Int(value) => {
                    row.set_item(cell.name, value)?;
                }
                ommx::experiment::ParameterValue::Float(value) => {
                    row.set_item(cell.name, value)?;
                }
                ommx::experiment::ParameterValue::String(value) => {
                    row.set_item(cell.name, value)?;
                }
            }
        }

        let entries = rows
            .into_values()
            .map(|row| row.into_any())
            .collect::<Vec<_>>();
        Ok(raw_entries_to_dataframe(py, entries, "run_id")?)
    }

    pub fn __repr__(&self) -> Result<String> {
        Ok(format!(
            "Experiment(image_name='{}', state='{}', open_runs={})",
            self.image_name()?,
            self.inner.state_name(),
            self.inner.open_run_count(),
        ))
    }
}

impl PyExperiment {
    fn commit_inner(&mut self, py: Python<'_>) -> Result<PyArtifact> {
        let _guard = crate::TRACING.attach_parent_context(py);
        Ok(PyArtifact::new(self.inner.commit()?))
    }
}

fn start_python_span(py: Python<'_>, name: &str) -> Result<Py<PyAny>> {
    let trace = py.import("opentelemetry.trace")?;
    let tracer = trace.call_method1("get_tracer", ("ommx.experiment",))?;
    let cm = tracer.call_method1("start_as_current_span", (name,))?;
    cm.call_method0("__enter__")?;
    Ok(cm.unbind())
}

fn set_current_span_run_id(py: Python<'_>, run_id: u64) -> Result<()> {
    let trace = py.import("opentelemetry.trace")?;
    let span = trace.call_method0("get_current_span")?;
    span.call_method1("set_attribute", ("ommx.run.id", run_id))?;
    Ok(())
}

fn python_exception_reason(
    exc_type: Option<&Bound<'_, PyAny>>,
    exc_value: Option<&Bound<'_, PyAny>>,
) -> String {
    let type_name = exc_type
        .and_then(|value| value.getattr("__name__").ok())
        .and_then(|value| value.extract::<String>().ok())
        .unwrap_or_else(|| "exception".to_string());
    match exc_value.and_then(|value| value.str().ok()) {
        Some(value) => {
            let message = value.to_string_lossy();
            if message.is_empty() {
                type_name
            } else {
                format!("{type_name}: {message}")
            }
        }
        None => type_name,
    }
}

fn is_keyboard_interrupt(py: Python<'_>, exc_type: Option<&Bound<'_, PyAny>>) -> Result<bool> {
    let Some(exc_type) = exc_type else {
        return Ok(false);
    };
    let Ok(exc_type) = exc_type.extract::<Py<PyType>>() else {
        return Ok(false);
    };
    Ok(exc_type
        .bind(py)
        .is_subclass(&py.get_type::<PyKeyboardInterrupt>())?)
}

fn close_python_context_manager(
    py: Python<'_>,
    cm: Option<&Py<PyAny>>,
    exc_type: Option<&Bound<'_, PyAny>>,
    exc_value: Option<&Bound<'_, PyAny>>,
    traceback: Option<&Bound<'_, PyAny>>,
) -> Result<()> {
    if let Some(cm) = cm {
        cm.bind(py)
            .call_method1("__exit__", (exc_type, exc_value, traceback))?;
    }
    Ok(())
}

pub struct AttachmentCodecInput(Py<PyType>);

impl AttachmentCodecInput {
    fn media_type(&self, py: Python<'_>) -> Result<MediaType> {
        let media_type: String = self
            .0
            .bind(py)
            .getattr("media_type")
            .context("Attachment codec class must define `media_type`")?
            .extract()
            .context("Attachment codec `media_type` must be a string")?;
        Ok(MediaType::from(media_type.as_str()))
    }

    fn encode(&self, py: Python<'_>, value: &AttachmentPayload) -> Result<EncodedAttachment> {
        let media_type = self.media_type(py)?;
        let value = value.0.bind(py);
        let bytes = self
            .0
            .bind(py)
            .call_method1("encode", (value,))
            .context("Attachment codec `encode(...)` failed")?
            .extract()
            .context("Attachment codec `encode(...)` must return bytes")?;
        Ok(EncodedAttachment { media_type, bytes })
    }

    fn decode(&self, py: Python<'_>, blob: &[u8]) -> Result<AttachmentPayload> {
        let value = self
            .0
            .bind(py)
            .call_method1("decode", (PyBytes::new(py, blob),))
            .context("Attachment codec `decode(...)` failed")?;
        Ok(AttachmentPayload(value.unbind()))
    }

    fn decode_from_parts(
        &self,
        py: Python<'_>,
        media_type: MediaType,
        blob: &[u8],
    ) -> Result<AttachmentPayload> {
        let expected = self.media_type(py)?;
        ensure_media_type(&media_type, &expected)?;
        self.decode(py, blob)
    }
}

pub struct AttachmentPayload(Py<PyAny>);

impl<'py> FromPyObject<'_, 'py> for AttachmentPayload {
    type Error = PyErr;

    fn extract(ob: pyo3::Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
        Ok(Self(ob.to_owned().unbind()))
    }
}

impl<'py> pyo3::IntoPyObject<'py> for AttachmentPayload {
    type Target = PyAny;
    type Output = Bound<'py, PyAny>;
    type Error = std::convert::Infallible;

    fn into_pyobject(self, py: Python<'py>) -> std::result::Result<Self::Output, Self::Error> {
        Ok(self.0.into_bound(py))
    }
}

impl pyo3_stub_gen::PyStubType for AttachmentPayload {
    fn type_input() -> pyo3_stub_gen::TypeInfo {
        pyo3_stub_gen::TypeInfo {
            name: "attachments.T".to_string(),
            source_module: None,
            import: ["ommx.experiment.attachments".into()].into(),
            type_refs: Default::default(),
        }
    }

    fn type_output() -> pyo3_stub_gen::TypeInfo {
        Self::type_input()
    }
}

struct EncodedAttachment {
    media_type: MediaType,
    bytes: Vec<u8>,
}

impl<'py> FromPyObject<'_, 'py> for AttachmentCodecInput {
    type Error = PyErr;

    fn extract(ob: pyo3::Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
        let codec = ob.extract::<Py<PyType>>().map_err(|_| {
            pyo3::exceptions::PyTypeError::new_err(
                "codec must be a class implementing ommx.experiment.attachments.AttachmentCodec",
            )
        })?;
        Ok(Self(codec))
    }
}

impl pyo3_stub_gen::PyStubType for AttachmentCodecInput {
    fn type_input() -> pyo3_stub_gen::TypeInfo {
        pyo3_stub_gen::TypeInfo {
            name: "type[attachments.AttachmentCodec[attachments.T]]".to_string(),
            source_module: None,
            import: ["ommx.experiment.attachments".into()].into(),
            type_refs: Default::default(),
        }
    }

    fn type_output() -> pyo3_stub_gen::TypeInfo {
        Self::type_input()
    }
}

fn ensure_media_type(actual: &MediaType, expected: &MediaType) -> Result<()> {
    anyhow::ensure!(
        actual == expected,
        "Expected media type '{expected}', got '{actual}'"
    );
    Ok(())
}

fn decode_json_blob<'py>(py: Python<'py>, blob: &[u8]) -> Result<Bound<'py, PyAny>> {
    let json = py.import("json")?;
    Ok(json.call_method1("loads", (PyBytes::new(py, &blob),))?)
}

fn decode_experiment_attachment<'py>(
    py: Python<'py>,
    experiment: &ommx::experiment::ExperimentDyn,
    name: &str,
) -> Result<Bound<'py, PyAny>> {
    match experiment.attachment_media_type(name)?.as_ref() {
        "application/json" => decode_json_blob(py, &experiment.attachment_blob(name)?),
        ommx::artifact::media_types::V1_INSTANCE_MEDIA_TYPE => {
            let (inner, annotations) = experiment.attachment_instance(name)?;
            Ok(crate::Instance { inner, annotations }
                .into_pyobject(py)?
                .into_any()
                .unbind()
                .into_bound(py))
        }
        ommx::artifact::media_types::V1_PARAMETRIC_INSTANCE_MEDIA_TYPE => {
            let (inner, annotations) = experiment.attachment_parametric_instance(name)?;
            Ok(crate::ParametricInstance { inner, annotations }
                .into_pyobject(py)?
                .into_any()
                .unbind()
                .into_bound(py))
        }
        ommx::artifact::media_types::V1_SOLUTION_MEDIA_TYPE => {
            let (inner, annotations) = experiment.attachment_solution(name)?;
            Ok(crate::Solution { inner, annotations }
                .into_pyobject(py)?
                .into_any()
                .unbind()
                .into_bound(py))
        }
        ommx::artifact::media_types::V1_SAMPLE_SET_MEDIA_TYPE => {
            let (inner, annotations) = experiment.attachment_sample_set(name)?;
            Ok(crate::SampleSet { inner, annotations }
                .into_pyobject(py)?
                .into_any()
                .unbind()
                .into_bound(py))
        }
        _ => Ok(PyBytes::new(py, &experiment.attachment_blob(name)?).into_any()),
    }
}

fn decode_run_attachment<'py>(
    py: Python<'py>,
    run: &ommx::experiment::SealedRunDyn,
    name: &str,
) -> Result<Bound<'py, PyAny>> {
    match run.attachment_media_type(name)?.as_ref() {
        "application/json" => decode_json_blob(py, &run.attachment_blob(name)?),
        ommx::artifact::media_types::V1_INSTANCE_MEDIA_TYPE => {
            let (inner, annotations) = run.attachment_instance(name)?;
            Ok(crate::Instance { inner, annotations }
                .into_pyobject(py)?
                .into_any()
                .unbind()
                .into_bound(py))
        }
        ommx::artifact::media_types::V1_PARAMETRIC_INSTANCE_MEDIA_TYPE => {
            let (inner, annotations) = run.attachment_parametric_instance(name)?;
            Ok(crate::ParametricInstance { inner, annotations }
                .into_pyobject(py)?
                .into_any()
                .unbind()
                .into_bound(py))
        }
        ommx::artifact::media_types::V1_SOLUTION_MEDIA_TYPE => {
            let (inner, annotations) = run.attachment_solution(name)?;
            Ok(crate::Solution { inner, annotations }
                .into_pyobject(py)?
                .into_any()
                .unbind()
                .into_bound(py))
        }
        ommx::artifact::media_types::V1_SAMPLE_SET_MEDIA_TYPE => {
            let (inner, annotations) = run.attachment_sample_set(name)?;
            Ok(crate::SampleSet { inner, annotations }
                .into_pyobject(py)?
                .into_any()
                .unbind()
                .into_bound(py))
        }
        _ => Ok(PyBytes::new(py, &run.attachment_blob(name)?).into_any()),
    }
}

#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[pyo3(module = "ommx._ommx_rust", name = "Run")]
/// A mutable run being recorded inside an unsealed Experiment.
///
/// A run represents one experimental condition or trial. Use
/// `log_parameter` for scalar values that should appear in
/// `Experiment.run_parameters_df`, and use attachment methods for payloads
/// such as JSON, instances, solutions, sample sets, or raw bytes.
///
/// Runs are usually created with `Experiment.run()` and used as context
/// managers. On normal context-manager exit the run is finished and added
/// to the parent experiment. On exception the run is closed as failed and
/// added with its partial state. `KeyboardInterrupt` is recorded separately as
/// `"interrupted"`. A run becomes immutable once it is closed.
pub struct PyRun {
    state: PyRunState,
    store_trace: bool,
}

enum PyRunState {
    /// Created by `Experiment.run()` and not yet entered as a context manager.
    Open { run: ommx::experiment::RunDyn },
    /// Inside `with run:`. The Python context manager must be closed before
    /// the Rust run can be finished or abandoned.
    Entered {
        run: ommx::experiment::RunDyn,
        span_context_manager: Py<PyAny>,
        trace_result: Option<Py<PyAny>>,
    },
    /// Finished or abandoned; no further mutation is allowed.
    Closed,
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl PyRun {
    pub fn __enter__(slf: Bound<'_, Self>) -> PyResult<Py<PyRun>> {
        {
            let py = slf.py();
            let mut this = slf.borrow_mut();
            let run = match mem::replace(&mut this.state, PyRunState::Closed) {
                PyRunState::Open { run } => run,
                state @ PyRunState::Entered { .. } => {
                    this.state = state;
                    return Err(anyhow::anyhow!("Run context has already been entered").into());
                }
                PyRunState::Closed => {
                    return Err(anyhow::anyhow!("Run has already been finished").into());
                }
            };
            let run_id = match run.run_id() {
                Ok(run_id) => run_id,
                Err(error) => {
                    this.state = PyRunState::Open { run };
                    return Err(error.into());
                }
            };
            let enter_result = if this.store_trace {
                start_run_trace_capture(py)
            } else {
                start_run_span(py)
            };
            let (span_context_manager, trace_result) = match enter_result {
                Ok(context) => context,
                Err(error) => {
                    this.state = PyRunState::Open { run };
                    return Err(error.into());
                }
            };
            if let Err(error) = set_current_span_run_id(py, run_id) {
                let close_result =
                    close_run_context_after_failed_enter(py, span_context_manager, &error);
                this.state = PyRunState::Open { run };
                close_result?;
                return Err(error.into());
            }
            this.state = PyRunState::Entered {
                run,
                span_context_manager,
                trace_result,
            };
        }
        Ok(slf.unbind())
    }

    #[pyo3(signature = (exc_type = None, exc_value = None, traceback = None))]
    pub fn __exit__(
        &mut self,
        py: Python<'_>,
        exc_type: Option<&Bound<'_, PyAny>>,
        exc_value: Option<&Bound<'_, PyAny>>,
        traceback: Option<&Bound<'_, PyAny>>,
    ) -> Result<bool> {
        match mem::replace(&mut self.state, PyRunState::Closed) {
            PyRunState::Closed => Ok(false),
            state @ PyRunState::Open { .. } => {
                self.state = state;
                anyhow::bail!("Run context has not been entered")
            }
            PyRunState::Entered {
                mut run,
                span_context_manager,
                trace_result,
            } => {
                let close_result = close_python_context_manager(
                    py,
                    Some(&span_context_manager),
                    exc_type,
                    exc_value,
                    traceback,
                );
                if let Err(error) = close_result {
                    self.state = PyRunState::Entered {
                        run,
                        span_context_manager,
                        trace_result,
                    };
                    return Err(error);
                }
                if exc_type.is_some() {
                    if self.store_trace {
                        if let Err(error) = store_trace_result(py, &mut run, trace_result) {
                            tracing::warn!(
                                error = %error,
                                "Failed to store Run trace during exception exit"
                            );
                        }
                    }
                    let finish_result = if is_keyboard_interrupt(py, exc_type)? {
                        run.finish_interrupted()
                    } else {
                        run.finish_failed()
                    };
                    if let Err(error) = finish_result {
                        tracing::warn!(
                            error = %error,
                            "Failed to close Run during exception exit"
                        );
                    }
                    return Ok(false);
                }
                if self.store_trace {
                    if let Err(error) = store_trace_result(py, &mut run, trace_result) {
                        run.abandon();
                        return Err(error);
                    }
                }
                run.finish()?;
                Ok(false)
            }
        }
    }

    #[getter]
    /// Integer identifier of this run within its Experiment.
    pub fn run_id(&self) -> Result<u64> {
        self.as_open()?.run_id()
    }

    /// Log a scalar parameter for this run.
    ///
    /// Accepted value types are `bool`, `int`, `float`, and `str`. These
    /// values are intended for comparing runs and are exposed as columns in
    /// `Experiment.run_parameters_df()`.
    pub fn log_parameter(
        &mut self,
        py: Python<'_>,
        name: &str,
        value: ParameterValueInput,
    ) -> Result<()> {
        let _guard = crate::TRACING.attach_parent_context(py);
        self.ensure_store_trace_context_started()?;
        self.as_open_mut()?.log_parameter(name, value.0)?;
        tracing::info!(parameter_name = %name, "ommx.run.parameter.recorded");
        Ok(())
    }

    /// Attach arbitrary bytes with an explicit OCI media type in this run.
    ///
    /// Use this for payloads that belong to this run but are not scalar run
    /// parameters, for example solver logs or derived files.
    pub fn log_attachment(
        &mut self,
        name: &str,
        media_type: &str,
        bytes: &Bound<pyo3::types::PyBytes>,
    ) -> Result<()> {
        self.ensure_store_trace_context_started()?;
        AttachmentLogger::log_attachment(
            self.as_open_mut()?,
            name,
            MediaType::from(media_type),
            bytes.as_bytes(),
            HashMap::new(),
        )
    }

    /// Attach an existing filesystem file in this run.
    ///
    /// The file bytes are copied into the Local Registry immediately. If
    /// `media_type` is omitted, the Rust SDK infers it from file contents and
    /// unknown types fall back to `application/octet-stream`. The original
    /// source path is not stored; only a basename for later export is stored
    /// as attachment metadata.
    #[pyo3(signature = (name, path, media_type = None, *, filename = None))]
    pub fn log_file(
        &mut self,
        py: Python<'_>,
        name: &str,
        path: PathBuf,
        media_type: Option<&str>,
        filename: Option<&str>,
    ) -> Result<()> {
        let _guard = crate::TRACING.attach_parent_context(py);
        self.ensure_store_trace_context_started()?;
        AttachmentLogger::log_file(
            self.as_open_mut()?,
            name,
            &path,
            media_type.map(MediaType::from),
            filename,
        )
    }

    /// Encode a Python object with an attachment codec and attach it in this run.
    ///
    /// The codec class must provide `media_type`, `encode(value) -> bytes`,
    /// and `decode(bytes) -> object`. OMMX owns only this protocol; concrete
    /// codecs should live in the package that owns the payload type.
    pub fn log_with_codec(
        &mut self,
        py: Python<'_>,
        codec: AttachmentCodecInput,
        name: &str,
        value: AttachmentPayload,
    ) -> Result<()> {
        let _guard = crate::TRACING.attach_parent_context(py);
        self.ensure_store_trace_context_started()?;
        let attachment = codec.encode(py, &value)?;
        AttachmentLogger::log_attachment(
            self.as_open_mut()?,
            name,
            attachment.media_type,
            attachment.bytes,
            HashMap::new(),
        )
    }

    /// Attach a JSON-serializable value in this run.
    ///
    /// The value is encoded with Python's `json.dumps` and stored with media
    /// type `application/json`.
    pub fn log_json(&mut self, py: Python<'_>, name: &str, value: &Bound<PyAny>) -> Result<()> {
        self.ensure_store_trace_context_started()?;
        let json = py.import("json")?;
        let blob: String = json.call_method1("dumps", (value,))?.extract()?;
        AttachmentLogger::log_attachment(
            self.as_open_mut()?,
            name,
            MediaType::from("application/json"),
            blob,
            HashMap::new(),
        )
    }

    /// Attach an Instance in this run.
    ///
    /// This records an instance as a run-level attachment. Use `log_solve`
    /// when the instance is the input of a solver call and should be paired
    /// with the returned solution.
    pub fn log_instance(&mut self, name: &str, instance: &crate::Instance) -> Result<()> {
        self.ensure_store_trace_context_started()?;
        AttachmentLogger::log_instance(
            self.as_open_mut()?,
            name,
            &instance.inner,
            instance.annotations.clone(),
        )
    }

    /// Attach a ParametricInstance in this run.
    pub fn log_parametric_instance(
        &mut self,
        name: &str,
        pi: &crate::ParametricInstance,
    ) -> Result<()> {
        self.ensure_store_trace_context_started()?;
        AttachmentLogger::log_parametric_instance(
            self.as_open_mut()?,
            name,
            &pi.inner,
            pi.annotations.clone(),
        )
    }

    /// Attach a Solution in this run.
    ///
    /// This records a solution as a run-level attachment. Use `log_solve`
    /// when the solution is produced by a solver call and should be paired
    /// with the input instance.
    pub fn log_solution(&mut self, name: &str, solution: &crate::Solution) -> Result<()> {
        self.ensure_store_trace_context_started()?;
        AttachmentLogger::log_solution(
            self.as_open_mut()?,
            name,
            &solution.inner,
            solution.annotations.clone(),
        )
    }

    /// Attach a SampleSet in this run.
    pub fn log_sample_set(&mut self, name: &str, sample_set: &crate::SampleSet) -> Result<()> {
        self.ensure_store_trace_context_started()?;
        AttachmentLogger::log_sample_set(
            self.as_open_mut()?,
            name,
            &sample_set.inner,
            sample_set.annotations.clone(),
        )
    }

    /// Solve an Instance with an OMMX SolverAdapter and log a Solve entry.
    ///
    /// The input Instance is cloned before calling the adapter, so adapter-side
    /// capability reductions do not mutate the caller's object. The original
    /// input is always stored as the Solve input.
    ///
    /// `adapter` must be a subclass of `ommx.adapter.SolverAdapter`. Keyword
    /// arguments are passed to `adapter.solve(...)` and recorded as
    /// `Solve.adapter_options`. The adapter class name is stored in
    /// `Solve.adapter`.
    ///
    /// Adapter options are solve-scoped metadata, not run parameters. They do
    /// not appear in `Experiment.run_parameters_df()`.
    #[pyo3(signature = (adapter, instance, **kwargs))]
    pub fn log_solve(
        &mut self,
        py: Python<'_>,
        adapter: SolverAdapterInput,
        instance: &crate::Instance,
        kwargs: Option<&Bound<PyDict>>,
    ) -> Result<crate::Solution> {
        let _guard = crate::TRACING.attach_parent_context(py);
        self.ensure_store_trace_context_started()?;
        reject_reserved_log_solve_kwargs(kwargs)?;
        let adapter = adapter.bind(py);
        let adapter_name = adapter.name()?;
        let adapter_options = dump_kwargs(py, kwargs)?;
        let diagnostics_collector = adapter.diagnostic_collector()?;
        let solution = adapter.solve(
            instance,
            kwargs,
            diagnostics_collector
                .as_ref()
                .map(|collector| collector.bind(py)),
        )?;
        let diagnostics = diagnostics_collector
            .as_ref()
            .map(|collector| collector.bind(py).borrow().pack(py))
            .transpose()?
            .flatten();
        let solve_id = self
            .as_open_mut()?
            .log_finished_solve_result_with_diagnostics(
                &instance.inner,
                instance.annotations.clone(),
                &solution.inner,
                solution.annotations.clone(),
                adapter_name,
                adapter_options,
                diagnostics,
            )?;
        tracing::info!(solve_id, "ommx.solve.recorded");
        Ok(solution)
    }

    /// Finish this run and append it to the parent Experiment.
    ///
    /// After this method returns, the run handle can no longer be used. The
    /// context manager calls this automatically on normal exit. On exception,
    /// the context manager closes the run as failed or interrupted with its
    /// partial state.
    pub fn finish(&mut self) -> Result<()> {
        if self.store_trace {
            anyhow::bail!(
                "store_trace=True requires using Run as a context manager; \
                 finish() is performed automatically on normal Run context-manager exit"
            );
        }
        self.finish_inner()
    }

    pub fn __repr__(&self) -> Result<String> {
        Ok(match &self.state {
            PyRunState::Open { run } | PyRunState::Entered { run, .. } => {
                format!("Run(run_id={})", run.run_id()?)
            }
            PyRunState::Closed => "Run(finished=True)".to_string(),
        })
    }
}

impl PyRun {
    fn ensure_store_trace_context_started(&self) -> Result<()> {
        if self.store_trace && matches!(self.state, PyRunState::Open { .. }) {
            anyhow::bail!("store_trace=True requires using Run as a context manager");
        }
        Ok(())
    }

    fn finish_inner(&mut self) -> Result<()> {
        let state = mem::replace(&mut self.state, PyRunState::Closed);
        match state {
            PyRunState::Open { run } => run.finish(),
            state @ PyRunState::Entered { .. } => {
                self.state = state;
                anyhow::bail!(
                    "Run context is active; finish() is performed automatically on normal Run context-manager exit"
                )
            }
            PyRunState::Closed => anyhow::bail!("Run has already been finished"),
        }
    }

    fn as_open(&self) -> Result<&ommx::experiment::RunDyn> {
        match &self.state {
            PyRunState::Open { run } | PyRunState::Entered { run, .. } => Ok(run),
            PyRunState::Closed => anyhow::bail!("Run has already been finished"),
        }
    }

    fn as_open_mut(&mut self) -> Result<&mut ommx::experiment::RunDyn> {
        match &mut self.state {
            PyRunState::Open { run } | PyRunState::Entered { run, .. } => Ok(run),
            PyRunState::Closed => anyhow::bail!("Run has already been finished"),
        }
    }
}

fn start_run_trace_capture(py: Python<'_>) -> Result<(Py<PyAny>, Option<Py<PyAny>>)> {
    let tracing = py.import("ommx.tracing")?;
    let cm = tracing
        .getattr("capture_trace")?
        .call1(("Run", "ommx.experiment"))?;
    let result = cm.call_method0("__enter__")?;
    Ok((cm.unbind(), Some(result.unbind())))
}

fn start_run_span(py: Python<'_>) -> Result<(Py<PyAny>, Option<Py<PyAny>>)> {
    Ok((start_python_span(py, "Run")?, None))
}

fn store_trace_result(
    py: Python<'_>,
    run: &mut ommx::experiment::RunDyn,
    trace_result: Option<Py<PyAny>>,
) -> Result<()> {
    let result = trace_result.context("store_trace=True lost its TraceResult before Run exit")?;
    let payload: Vec<u8> = result.bind(py).call_method0("otlp_protobuf")?.extract()?;
    run.store_trace(ommx::experiment::Trace::from_bytes(payload))?;
    Ok(())
}

fn close_run_context_after_failed_enter(
    py: Python<'_>,
    span_context_manager: Py<PyAny>,
    original_error: &anyhow::Error,
) -> Result<()> {
    let original_message = original_error.to_string();
    close_python_context_manager(py, Some(&span_context_manager), None, None, None)
        .with_context(|| {
            format!(
                "Run context setup failed with `{original_message}`, then closing the partial context failed"
            )
        })
}

fn parse_name(image_name: Option<&str>) -> Result<ommx::experiment::Name> {
    match image_name {
        Some(image_name) => Ok(ommx::experiment::Name::Named(
            ommx::artifact::ImageRef::parse(image_name)?,
        )),
        None => Ok(ommx::experiment::Name::Anonymous),
    }
}

pub struct ParameterValueInput(ommx::experiment::ParameterValue);

impl<'py> FromPyObject<'_, 'py> for ParameterValueInput {
    type Error = PyErr;

    fn extract(ob: pyo3::Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
        if ob.is_instance_of::<PyBool>() {
            let value = ob.extract::<bool>()?;
            return Ok(Self(ommx::experiment::ParameterValue::Bool(value)));
        }
        if ob.is_instance_of::<PyInt>() {
            let value = ob.extract::<i64>().map_err(|_| {
                pyo3::exceptions::PyOverflowError::new_err(
                    "Run parameter int value must fit in int64",
                )
            })?;
            return Ok(Self(ommx::experiment::ParameterValue::Int(value)));
        }
        if ob.is_instance_of::<PyFloat>() {
            let value = ob.extract::<f64>()?;
            return Ok(Self(ommx::experiment::ParameterValue::Float(value)));
        }
        if ob.is_instance_of::<PyString>() {
            let value = ob.extract::<String>()?;
            return Ok(Self(ommx::experiment::ParameterValue::String(value)));
        }
        Err(pyo3::exceptions::PyTypeError::new_err(
            "Run parameter value must be bool, int, float, or str",
        ))
    }
}

pub struct SolverAdapterInput(Py<PyType>);

impl SolverAdapterInput {
    fn bind<'py>(&'py self, py: Python<'py>) -> SolverAdapter<'py> {
        SolverAdapter {
            adapter: self.0.bind(py),
        }
    }
}

#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[pyo3(module = "ommx._ommx_rust", name = "DiagnosticCollector")]
pub struct PyDiagnosticCollector {
    diagnostics: Vec<DiagnosticReport>,
}

impl PyDiagnosticCollector {
    fn new_inner() -> Self {
        Self {
            diagnostics: Vec::new(),
        }
    }

    fn pack(&self, py: Python<'_>) -> Result<Option<SolveDiagnosticPayload>> {
        let dataclasses = py.import("dataclasses")?;
        let msgpack = py.import("msgpack")?;
        let mut packed_items = Vec::new();
        let mut python_types = Vec::new();
        for diagnostic in &self.diagnostics {
            let diagnostic = diagnostic.as_bound(py);
            let type_name = python_type_name(diagnostic)?;
            let data = dataclasses
                .call_method1("asdict", (diagnostic,))
                .with_context(|| {
                    format!("Adapter diagnostic `{type_name}` must be a dataclass instance")
                })?;
            packed_items.push(data);
            python_types.push(type_name);
        }
        if packed_items.is_empty() {
            return Ok(None);
        }
        let diagnostics = PyList::new(py, packed_items)?;
        let kwargs = PyDict::new(py);
        kwargs.set_item("use_bin_type", true)?;
        let bytes: Vec<u8> = msgpack
            .call_method("packb", (&diagnostics,), Some(&kwargs))?
            .extract()?;
        let mut annotations = HashMap::new();
        annotations.insert(
            "org.ommx.diagnostic.python_type".to_string(),
            "builtins.list".to_string(),
        );
        annotations.insert(
            "org.ommx.diagnostic.python_element_types".to_string(),
            python_types.join(","),
        );
        Ok(Some(SolveDiagnosticPayload::new(
            media_types::diagnostic_msgpack(),
            bytes,
            annotations,
        )))
    }
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl PyDiagnosticCollector {
    #[new]
    pub fn new() -> Self {
        Self::new_inner()
    }

    #[getter]
    pub fn diagnostics(&self, py: Python<'_>) -> Vec<DiagnosticReport> {
        self.diagnostics
            .iter()
            .map(|diagnostic| diagnostic.clone_ref(py))
            .collect()
    }

    pub fn record(&mut self, diagnostic: DiagnosticReport) {
        self.diagnostics.push(diagnostic);
    }
}

pub struct DiagnosticReport(Py<PyAny>);

impl DiagnosticReport {
    fn as_bound<'py>(&self, py: Python<'py>) -> &Bound<'py, PyAny> {
        self.0.bind(py)
    }

    fn clone_ref(&self, py: Python<'_>) -> Self {
        Self(self.0.clone_ref(py))
    }
}

impl<'py> FromPyObject<'_, 'py> for DiagnosticReport {
    type Error = PyErr;

    fn extract(ob: pyo3::Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
        let py = ob.py();
        let is_dataclass: bool = py
            .import("dataclasses")?
            .call_method1("is_dataclass", (&ob,))?
            .extract()?;
        if !is_dataclass || ob.is_instance_of::<PyType>() {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "diagnostic must be a dataclass instance",
            ));
        }
        Ok(Self(ob.to_owned().unbind()))
    }
}

impl<'py> pyo3::IntoPyObject<'py> for DiagnosticReport {
    type Target = PyAny;
    type Output = Bound<'py, PyAny>;
    type Error = std::convert::Infallible;

    fn into_pyobject(self, py: Python<'py>) -> std::result::Result<Self::Output, Self::Error> {
        Ok(self.0.into_bound(py))
    }
}

impl pyo3_stub_gen::PyStubType for DiagnosticReport {
    fn type_input() -> pyo3_stub_gen::TypeInfo {
        pyo3_stub_gen::TypeInfo {
            name: "adapter.DiagnosticReport".to_string(),
            source_module: None,
            import: ["ommx.adapter".into()].into(),
            type_refs: Default::default(),
        }
    }

    fn type_output() -> pyo3_stub_gen::TypeInfo {
        Self::type_input()
    }
}

struct SolverAdapter<'py> {
    adapter: &'py Bound<'py, PyType>,
}

impl<'py> SolverAdapter<'py> {
    fn py(&self) -> Python<'py> {
        self.adapter.py()
    }

    fn solve(
        &self,
        instance: &crate::Instance,
        kwargs: Option<&Bound<'py, PyDict>>,
        diagnostics: Option<&Bound<'py, PyDiagnosticCollector>>,
    ) -> Result<crate::Solution> {
        let py = self.py();
        let adapter_instance = Py::new(py, instance.clone())?;
        let solution_object = match diagnostics {
            Some(diagnostics) => {
                let call_kwargs = PyDict::new(py);
                if let Some(kwargs) = kwargs {
                    for (key, value) in kwargs.iter() {
                        call_kwargs.set_item(key, value)?;
                    }
                }
                call_kwargs.set_item("diagnostics", diagnostics)?;
                self.adapter
                    .call_method("solve", (adapter_instance,), Some(&call_kwargs))?
            }
            None => self
                .adapter
                .call_method("solve", (adapter_instance,), kwargs)?,
        };
        solution_object
            .extract::<crate::Solution>()
            .map_err(|_| anyhow::anyhow!("adapter.solve(...) must return ommx.v1.Solution"))
    }

    fn diagnostic_collector(&self) -> Result<Option<Py<PyDiagnosticCollector>>> {
        if !self.supports_diagnostics()? {
            return Ok(None);
        }
        Ok(Some(Py::new(
            self.py(),
            PyDiagnosticCollector::new_inner(),
        )?))
    }

    fn supports_diagnostics(&self) -> Result<bool> {
        let value = self
            .adapter
            .getattr("SUPPORTS_DIAGNOSTICS")
            .context("SolverAdapter.SUPPORTS_DIAGNOSTICS must be readable")?;
        Ok(value.extract::<bool>()?)
    }

    fn name(&self) -> Result<String> {
        let module: String = self.adapter.module()?.extract()?;
        let qualname: String = self.adapter.qualname()?.extract()?;
        Ok(format!("{module}.{qualname}"))
    }
}

fn dump_kwargs(py: Python<'_>, kwargs: Option<&Bound<PyDict>>) -> Result<String> {
    let json = py.import("json")?;
    let encoded: String = match kwargs {
        Some(kwargs) => json.call_method1("dumps", (kwargs,)),
        None => json.call_method1("dumps", (PyDict::new(py),)),
    }
    .context("SolverAdapter kwargs must be JSON-serializable")?
    .extract()?;
    Ok(encoded)
}

fn reject_reserved_log_solve_kwargs(kwargs: Option<&Bound<PyDict>>) -> Result<()> {
    let Some(kwargs) = kwargs else {
        return Ok(());
    };
    let has_diagnostics: bool = kwargs
        .call_method1("__contains__", ("diagnostics",))?
        .extract()?;
    if has_diagnostics {
        anyhow::bail!("Run.log_solve owns the `diagnostics` adapter option");
    }
    Ok(())
}

fn python_type_name(value: &Bound<'_, PyAny>) -> Result<String> {
    let ty = value.get_type();
    let module: String = ty.getattr("__module__")?.extract()?;
    let qualname: String = ty.getattr("__qualname__")?.extract()?;
    Ok(format!("{module}.{qualname}"))
}

impl<'py> FromPyObject<'_, 'py> for SolverAdapterInput {
    type Error = PyErr;

    fn extract(ob: pyo3::Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
        let adapter = ob.extract::<Py<PyType>>().map_err(|_| {
            pyo3::exceptions::PyTypeError::new_err(
                "adapter must be a subclass of ommx.adapter.SolverAdapter",
            )
        })?;
        let adapter_bound = adapter.bind(ob.py());
        let solver_adapter = ob.py().import("ommx.adapter")?.getattr("SolverAdapter")?;
        if !adapter_bound.is_subclass(&solver_adapter)? {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "adapter must be a subclass of ommx.adapter.SolverAdapter",
            ));
        }
        Ok(Self(adapter))
    }
}

impl pyo3_stub_gen::PyStubType for SolverAdapterInput {
    fn type_input() -> pyo3_stub_gen::TypeInfo {
        pyo3_stub_gen::TypeInfo {
            name: "type[adapter.SolverAdapter]".to_string(),
            source_module: None,
            import: ["ommx.adapter".into()].into(),
            type_refs: Default::default(),
        }
    }

    fn type_output() -> pyo3_stub_gen::TypeInfo {
        Self::type_input()
    }
}

impl pyo3_stub_gen::PyStubType for ParameterValueInput {
    fn type_input() -> pyo3_stub_gen::TypeInfo {
        pyo3_stub_gen::TypeInfo {
            name: "bool | int | float | str".to_string(),
            source_module: None,
            import: Default::default(),
            type_refs: Default::default(),
        }
    }

    fn type_output() -> pyo3_stub_gen::TypeInfo {
        Self::type_input()
    }
}

#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[pyo3(module = "ommx._ommx_rust", name = "SealedRun")]
/// Immutable view of a closed Run in an Experiment.
///
/// `SealedRun` exposes run-level attachments by name and the sequence of
/// `Solve` records created by `Run.log_solve`. The `status` property is
/// `"finished"`, `"failed"`, or `"interrupted"` depending on how the Run was
/// closed.
pub struct PySealedRun {
    run: ommx::experiment::SealedRunDyn,
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl PySealedRun {
    #[getter]
    /// Integer identifier of this run within its Experiment.
    pub fn run_id(&self) -> u64 {
        self.run.run_id()
    }

    #[getter]
    /// Run lifecycle status: `"finished"`, `"failed"`, or `"interrupted"`.
    pub fn status(&self) -> String {
        self.run.status().to_string()
    }

    #[getter]
    /// Names of run-level attachments.
    pub fn attachment_names(&self) -> Result<Vec<String>> {
        Ok(self.run.attachment_names())
    }

    /// OCI media type of a run-level attachment.
    pub fn attachment_media_type(&self, name: &str) -> Result<String> {
        Ok(self.run.attachment_media_type(name)?.to_string())
    }

    /// Read a run-level attachment by name.
    ///
    /// The returned Python object is decoded from the attachment media type.
    pub fn get_attachment<'py>(&self, py: Python<'py>, name: &str) -> Result<Bound<'py, PyAny>> {
        let _guard = crate::TRACING.attach_parent_context(py);
        decode_run_attachment(py, &self.run, name)
    }

    /// Read a JSON run-level attachment by name.
    pub fn get_json<'py>(&self, py: Python<'py>, name: &str) -> Result<Bound<'py, PyAny>> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let expected = MediaType::from("application/json");
        ensure_media_type(&self.run.attachment_media_type(name)?, &expected)?;
        decode_json_blob(py, &self.run.attachment_blob(name)?)
    }

    /// Read an Instance run-level attachment by name.
    pub fn get_instance(&self, py: Python<'_>, name: &str) -> Result<crate::Instance> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let (inner, annotations) = self.run.attachment_instance(name)?;
        Ok(crate::Instance { inner, annotations })
    }

    /// Read a ParametricInstance run-level attachment by name.
    pub fn get_parametric_instance(
        &self,
        py: Python<'_>,
        name: &str,
    ) -> Result<crate::ParametricInstance> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let (inner, annotations) = self.run.attachment_parametric_instance(name)?;
        Ok(crate::ParametricInstance { inner, annotations })
    }

    /// Read a Solution run-level attachment by name.
    pub fn get_solution(&self, py: Python<'_>, name: &str) -> Result<crate::Solution> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let (inner, annotations) = self.run.attachment_solution(name)?;
        Ok(crate::Solution { inner, annotations })
    }

    /// Read a SampleSet run-level attachment by name.
    pub fn get_sample_set(&self, py: Python<'_>, name: &str) -> Result<crate::SampleSet> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let (inner, annotations) = self.run.attachment_sample_set(name)?;
        Ok(crate::SampleSet { inner, annotations })
    }

    /// Read raw bytes of a run-level attachment by name.
    pub fn get_blob<'py>(&self, py: Python<'py>, name: &str) -> Result<Bound<'py, PyBytes>> {
        let _guard = crate::TRACING.attach_parent_context(py);
        Ok(PyBytes::new(py, &self.run.attachment_blob(name)?))
    }

    /// Write a run-level attachment to a filesystem path.
    ///
    /// If `path` names an existing directory, the attachment filename stored
    /// by `log_file` is used inside that directory. Otherwise `path` is
    /// treated as the destination file path.
    #[pyo3(signature = (name, path, *, overwrite = false))]
    pub fn write_attachment(
        &self,
        py: Python<'_>,
        name: &str,
        path: PathBuf,
        overwrite: bool,
    ) -> Result<PathBuf> {
        let _guard = crate::TRACING.attach_parent_context(py);
        self.run.write_attachment(name, path, overwrite)
    }

    /// Read a run-level attachment by name and decode it with a codec.
    ///
    /// The codec class must provide `media_type`, `encode(value) -> bytes`,
    /// and `decode(bytes) -> object`. This method validates the stored media
    /// type against the codec before decoding.
    pub fn get_with_codec(
        &self,
        py: Python<'_>,
        codec: AttachmentCodecInput,
        name: &str,
    ) -> Result<AttachmentPayload> {
        let _guard = crate::TRACING.attach_parent_context(py);
        codec.decode_from_parts(
            py,
            self.run.attachment_media_type(name)?,
            &self.run.attachment_blob(name)?,
        )
    }

    #[getter]
    #[gen_stub(override_return_type(
        type_repr = "typing.Optional[tracing.TraceResult]",
        imports = ("typing", "ommx.tracing")
    ))]
    /// Stored trace for this run, or `None` when this run was recorded without trace storage.
    pub fn trace<'py>(&self, py: Python<'py>) -> Result<Option<Bound<'py, PyAny>>> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let Some(trace) = self.run.trace()? else {
            return Ok(None);
        };
        let trace_result = py.import("ommx.tracing")?.getattr("TraceResult")?;
        Ok(Some(trace_result.call_method1(
            "from_otlp_protobuf",
            (PyBytes::new(py, trace.as_bytes()),),
        )?))
    }

    #[getter]
    /// Solve records logged in this run, ordered by `solve_id`.
    pub fn solves(&self) -> Vec<PySolve> {
        self.run.solves().iter().cloned().map(PySolve).collect()
    }

    pub fn __repr__(&self) -> String {
        format!(
            "SealedRun(run_id={}, status='{}', attachments={}, solves={})",
            self.run_id(),
            self.status(),
            self.run.attachment_count(),
            self.run.solves().len(),
        )
    }
}

#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[pyo3(module = "ommx._ommx_rust", name = "Solve")]
#[derive(Clone)]
/// Immutable record of one solver call.
///
/// A `Solve` stores the input `Instance`, output `Solution`, adapter class
/// name, and JSON-encoded adapter options for one `Run.log_solve` call.
pub struct PySolve(ommx::experiment::SolveDyn);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl PySolve {
    #[getter]
    /// Integer identifier of this solve within its run.
    pub fn solve_id(&self) -> u64 {
        self.0.solve_id()
    }

    #[getter]
    /// Input `Instance` passed to the solver.
    pub fn input(&self) -> Result<crate::Instance> {
        let (inner, annotations) = self.0.input_instance()?;
        Ok(crate::Instance { inner, annotations })
    }

    #[getter]
    /// Output `Solution` returned by the solver.
    pub fn output(&self) -> Result<crate::Solution> {
        let (inner, annotations) = self.0.output_solution()?;
        Ok(crate::Solution { inner, annotations })
    }

    #[getter]
    /// SolverAdapter class name used for this solve.
    pub fn adapter(&self) -> String {
        self.0.adapter().to_string()
    }

    #[getter]
    #[gen_stub(override_return_type(
        type_repr = "builtins.dict[builtins.str, typing.Any]",
        imports = ("builtins", "typing")
    ))]
    /// Keyword arguments passed to the SolverAdapter.
    ///
    /// The artifact stores this value as a JSON string produced by Python's
    /// `json.dumps`; the Python SDK decodes it with `json.loads` before
    /// returning it.
    pub fn adapter_options<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyDict>> {
        let json = py.import("json")?;
        Ok(json
            .call_method1("loads", (self.0.adapter_options(),))?
            .cast::<PyDict>()
            .map_err(|_| anyhow::anyhow!("Solve.adapter_options must decode to a JSON object"))?
            .clone())
    }

    #[getter]
    #[pyo3(name = "diagnostics")]
    #[gen_stub(override_return_type(
        type_repr = "builtins.list[typing.Any]",
        imports = ("builtins", "typing")
    ))]
    /// Adapter-defined diagnostics recorded during this solve.
    pub fn diagnostics_property<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyList>> {
        unpack_diagnostics(py, &self.0)
    }

    pub fn __repr__(&self) -> String {
        format!("Solve(solve_id={})", self.solve_id())
    }
}

fn unpack_diagnostics<'py>(
    py: Python<'py>,
    solve: &ommx::experiment::SolveDyn,
) -> Result<Bound<'py, PyList>> {
    let Some(blob) = solve.diagnostic_blob()? else {
        return Ok(PyList::empty(py));
    };
    let msgpack = py.import("msgpack")?;
    let kwargs = PyDict::new(py);
    kwargs.set_item("raw", false)?;
    let decoded = msgpack.call_method("unpackb", (PyBytes::new(py, &blob),), Some(&kwargs))?;
    Ok(decoded
        .cast::<PyList>()
        .map_err(|_| anyhow::anyhow!("Solve diagnostics payload must decode to a list"))?
        .clone())
}
