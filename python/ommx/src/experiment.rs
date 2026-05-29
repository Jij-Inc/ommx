use anyhow::{Context, Result};
use oci_spec::image::{Descriptor, MediaType};
use pyo3::{
    prelude::*,
    types::{PyBool, PyBytes, PyDict, PyFloat, PyInt, PyString, PyType, PyTypeMethods},
};
use std::{
    collections::{btree_map::Entry, BTreeMap},
    path::PathBuf,
};

use crate::pandas::{raw_entries_to_dataframe, PyDataFrame};
use crate::{PyArtifact, PyDescriptor};
use ommx::artifact::AsArtifact;
use ommx::experiment::AttachmentLogger;

#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[pyo3(module = "ommx._ommx_rust", name = "Experiment")]
/// A collection of optimization experiment records stored as one OMMX Artifact.
///
/// An `Experiment` owns experiment-level attachments and a sequence of
/// finished `Run` objects. Each `Run` can store scalar run parameters,
/// run-level attachments, and zero or more `Solve` records.
///
/// Newly created experiments are unsealed. Call `commit()` to write the
/// experiment into the local registry as an OMMX Artifact. After commit, the
/// same object can be used as a read-only view of the committed artifact.
/// `with Experiment(...)` commits on normal exit if the experiment is still
/// unsealed, and does not auto-commit on exception.
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
/// >>> len(exp.experiment_attachments)
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
    context_entered: bool,
    trace_context_manager: Option<Py<PyAny>>,
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
            context_entered: false,
            trace_context_manager: None,
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
            context_entered: false,
            trace_context_manager: None,
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
            context_entered: false,
            trace_context_manager: None,
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
            context_entered: false,
            trace_context_manager: None,
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
            context_entered: false,
            trace_context_manager: None,
        })
    }

    /// Fork this committed Experiment into a new unsealed child Experiment.
    ///
    /// The parent Experiment is not modified. Existing Attachments, Runs,
    /// Solves, and Run parameters are carried into the child Experiment.
    /// When the child is committed, its Artifact manifest records the parent
    /// manifest descriptor as OCI `subject`.
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
            context_entered: false,
            trace_context_manager: None,
        })
    }

    pub fn __enter__(slf: Bound<'_, Self>) -> PyResult<Py<PyExperiment>> {
        {
            let py = slf.py();
            let mut this = slf.borrow_mut();
            this.enter_experiment_context(py)?;
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
        self.exit_experiment_context(py, exc_type, exc_value, traceback)?;
        if exc_type.is_none() && self.inner.is_unsealed() {
            self.commit_inner(py)?;
        }
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
    /// Low-level descriptors for experiment-level attachments.
    ///
    /// Prefer `attachment_names`, `get_attachment`, or typed methods such as
    /// `get_json` and `get_instance` when working from Python. This descriptor
    /// view is kept for low-level Artifact inspection.
    pub fn experiment_attachments(&self) -> Result<Vec<PyDescriptor>> {
        Ok(self
            .inner
            .experiment_attachments()?
            .into_iter()
            .map(PyDescriptor::from)
            .collect())
    }

    #[getter]
    /// Names of experiment-level attachments.
    pub fn attachment_names(&self) -> Result<Vec<String>> {
        Ok(self
            .inner
            .experiment_attachments()?
            .into_iter()
            .filter_map(|descriptor| attachment_name(&descriptor).map(ToOwned::to_owned))
            .collect())
    }

    /// Read an experiment-level attachment by name.
    ///
    /// The returned Python object is decoded from the attachment media type:
    /// JSON attachments become normal Python objects, OMMX instance-like
    /// attachments become the corresponding `ommx.v1` objects, and unknown
    /// media types are returned as raw `bytes`.
    pub fn get_attachment<'py>(&self, py: Python<'py>, name: &str) -> Result<Bound<'py, PyAny>> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let (artifact, descriptor) = self.find_attachment(name)?;
        decode_attachment(py, &artifact, &descriptor)
    }

    /// Read a JSON experiment-level attachment by name.
    ///
    /// Raises an error if the attachment exists but its media type is not
    /// `application/json`.
    pub fn get_json<'py>(&self, py: Python<'py>, name: &str) -> Result<Bound<'py, PyAny>> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let (artifact, descriptor) = self.find_attachment(name)?;
        decode_json_attachment(py, &artifact, &descriptor)
    }

    /// Read an Instance experiment-level attachment by name.
    pub fn get_instance(&self, py: Python<'_>, name: &str) -> Result<crate::Instance> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let (artifact, descriptor) = self.find_attachment(name)?;
        decode_instance_attachment(&artifact, &descriptor)
    }

    /// Read a ParametricInstance experiment-level attachment by name.
    pub fn get_parametric_instance(
        &self,
        py: Python<'_>,
        name: &str,
    ) -> Result<crate::ParametricInstance> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let (artifact, descriptor) = self.find_attachment(name)?;
        decode_parametric_instance_attachment(&artifact, &descriptor)
    }

    /// Read a Solution experiment-level attachment by name.
    pub fn get_solution(&self, py: Python<'_>, name: &str) -> Result<crate::Solution> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let (artifact, descriptor) = self.find_attachment(name)?;
        decode_solution_attachment(&artifact, &descriptor)
    }

    /// Read a SampleSet experiment-level attachment by name.
    pub fn get_sample_set(&self, py: Python<'_>, name: &str) -> Result<crate::SampleSet> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let (artifact, descriptor) = self.find_attachment(name)?;
        decode_sample_set_attachment(&artifact, &descriptor)
    }

    /// Read raw bytes of an experiment-level attachment by name.
    pub fn get_blob<'py>(&self, py: Python<'py>, name: &str) -> Result<Bound<'py, PyBytes>> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let (artifact, descriptor) = self.find_attachment(name)?;
        Ok(PyBytes::new(py, &artifact.get_blob(&descriptor)?))
    }

    #[getter]
    /// Finished runs in insertion order.
    pub fn runs(&self) -> Result<Vec<PySealedRun>> {
        let artifact = self.inner.artifact()?;
        Ok(self
            .inner
            .runs()?
            .into_iter()
            .map(|run| PySealedRun {
                run,
                artifact: artifact.clone(),
            })
            .collect())
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
    /// The returned `Run` must be finished before `commit()`. Use it as a
    /// context manager to finish it automatically on normal exit:
    ///
    /// ```python
    /// with experiment.run() as run:
    ///     run.log_parameter("capacity", 47)
    /// ```
    pub fn run(&self) -> Result<PyRun> {
        Ok(PyRun {
            run: Some(self.inner.run()?),
            store_trace: self.store_trace,
            context_entered: false,
            context_active: false,
            span_context_manager: None,
            trace_result: None,
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
            MediaType::Other(media_type.to_string()),
            bytes.as_bytes(),
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
            MediaType::Other("application/json".to_string()),
            blob,
        )
    }

    /// Attach an Instance in the experiment space.
    pub fn log_instance(&mut self, name: &str, instance: &crate::Instance) -> Result<()> {
        AttachmentLogger::log_instance(&self.inner, name, &instance.inner)
    }

    /// Attach an ParametricInstance in the experiment space.
    pub fn log_parametric_instance(
        &mut self,
        name: &str,
        pi: &crate::ParametricInstance,
    ) -> Result<()> {
        AttachmentLogger::log_parametric_instance(&self.inner, name, &pi.inner)
    }

    /// Attach a Solution in the experiment space.
    pub fn log_solution(&mut self, name: &str, solution: &crate::Solution) -> Result<()> {
        AttachmentLogger::log_solution(&self.inner, name, &solution.inner)
    }

    /// Attach a SampleSet in the experiment space.
    pub fn log_sample_set(&mut self, name: &str, sample_set: &crate::SampleSet) -> Result<()> {
        AttachmentLogger::log_sample_set(&self.inner, name, &sample_set.inner)
    }

    /// Commit this unsealed Experiment into the local registry.
    ///
    /// All open runs must be finished before committing. The returned
    /// `Artifact` can be saved as a `.ommx` archive or passed to
    /// `Experiment.from_artifact`. After commit, this object becomes a
    /// read-only view of the committed Experiment.
    pub fn commit(&mut self, py: Python<'_>) -> Result<PyArtifact> {
        self.commit_inner(py)
    }

    /// Wide DataFrame of run parameters, indexed by `run_id`.
    ///
    /// Run parameters are scalar values logged with `Run.log_parameter`.
    /// Completed runs with no parameters are still present as index rows.
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
    fn enter_experiment_context(&mut self, py: Python<'_>) -> Result<()> {
        if self.store_trace && self.context_entered {
            anyhow::bail!("Experiment context has already been entered");
        }
        self.context_entered = true;
        self.trace_context_manager = Some(start_python_span(py, "ommx.experiment")?);
        Ok(())
    }

    fn exit_experiment_context(
        &mut self,
        py: Python<'_>,
        exc_type: Option<&Bound<'_, PyAny>>,
        exc_value: Option<&Bound<'_, PyAny>>,
        traceback: Option<&Bound<'_, PyAny>>,
    ) -> Result<()> {
        let close_result = close_python_context_manager(
            py,
            self.trace_context_manager.take(),
            exc_type,
            exc_value,
            traceback,
        );
        close_result
    }

    fn commit_inner(&mut self, py: Python<'_>) -> Result<PyArtifact> {
        let _guard = crate::TRACING.attach_parent_context(py);
        Ok(PyArtifact::new(self.inner.commit()?))
    }

    fn find_attachment(
        &self,
        name: &str,
    ) -> Result<(ommx::artifact::LocalArtifactDyn, Descriptor)> {
        let artifact = self.inner.artifact()?;
        for descriptor in self.inner.experiment_attachments()? {
            if attachment_name(&descriptor) == Some(name) {
                return Ok((artifact, Descriptor::from(descriptor)));
            }
        }
        anyhow::bail!("Experiment attachment `{name}` not found")
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

fn close_python_context_manager(
    py: Python<'_>,
    cm: Option<Py<PyAny>>,
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

const ANN_ATTACHMENT_NAME: &str = "org.ommx.attachment.name";

fn attachment_name(descriptor: &Descriptor) -> Option<&str> {
    descriptor
        .annotations()
        .as_ref()
        .and_then(|annotations| annotations.get(ANN_ATTACHMENT_NAME))
        .map(String::as_str)
}

fn attachment_annotations(descriptor: &Descriptor) -> std::collections::HashMap<String, String> {
    descriptor
        .annotations()
        .as_ref()
        .cloned()
        .unwrap_or_default()
}

fn attachment_blob(
    artifact: &ommx::artifact::LocalArtifactDyn,
    descriptor: &Descriptor,
    expected_media_type: Option<&str>,
) -> Result<Vec<u8>> {
    if let Some(expected) = expected_media_type {
        let actual = descriptor.media_type().to_string();
        if actual != expected {
            anyhow::bail!("Expected media type '{expected}', got '{actual}'");
        }
    }
    artifact.get_blob(descriptor)
}

fn decode_attachment<'py>(
    py: Python<'py>,
    artifact: &ommx::artifact::LocalArtifactDyn,
    descriptor: &Descriptor,
) -> Result<Bound<'py, PyAny>> {
    match descriptor.media_type().as_ref() {
        "application/json" => decode_json_attachment(py, artifact, descriptor),
        ommx::artifact::media_types::V1_INSTANCE_MEDIA_TYPE => {
            let instance = decode_instance_attachment(artifact, descriptor)?;
            Ok(instance
                .into_pyobject(py)?
                .into_any()
                .unbind()
                .into_bound(py))
        }
        ommx::artifact::media_types::V1_PARAMETRIC_INSTANCE_MEDIA_TYPE => {
            let instance = decode_parametric_instance_attachment(artifact, descriptor)?;
            Ok(instance
                .into_pyobject(py)?
                .into_any()
                .unbind()
                .into_bound(py))
        }
        ommx::artifact::media_types::V1_SOLUTION_MEDIA_TYPE => {
            let solution = decode_solution_attachment(artifact, descriptor)?;
            Ok(solution
                .into_pyobject(py)?
                .into_any()
                .unbind()
                .into_bound(py))
        }
        ommx::artifact::media_types::V1_SAMPLE_SET_MEDIA_TYPE => {
            let sample_set = decode_sample_set_attachment(artifact, descriptor)?;
            Ok(sample_set
                .into_pyobject(py)?
                .into_any()
                .unbind()
                .into_bound(py))
        }
        _ => Ok(PyBytes::new(py, &attachment_blob(artifact, descriptor, None)?).into_any()),
    }
}

fn decode_json_attachment<'py>(
    py: Python<'py>,
    artifact: &ommx::artifact::LocalArtifactDyn,
    descriptor: &Descriptor,
) -> Result<Bound<'py, PyAny>> {
    let blob = attachment_blob(artifact, descriptor, Some("application/json"))?;
    let json = py.import("json")?;
    Ok(json.call_method1("loads", (PyBytes::new(py, &blob),))?)
}

fn decode_instance_attachment(
    artifact: &ommx::artifact::LocalArtifactDyn,
    descriptor: &Descriptor,
) -> Result<crate::Instance> {
    let blob = attachment_blob(
        artifact,
        descriptor,
        Some(ommx::artifact::media_types::V1_INSTANCE_MEDIA_TYPE),
    )?;
    Ok(crate::Instance {
        inner: ommx::Instance::from_bytes(&blob)?,
        annotations: attachment_annotations(descriptor),
    })
}

fn decode_parametric_instance_attachment(
    artifact: &ommx::artifact::LocalArtifactDyn,
    descriptor: &Descriptor,
) -> Result<crate::ParametricInstance> {
    let blob = attachment_blob(
        artifact,
        descriptor,
        Some(ommx::artifact::media_types::V1_PARAMETRIC_INSTANCE_MEDIA_TYPE),
    )?;
    Ok(crate::ParametricInstance {
        inner: ommx::ParametricInstance::from_bytes(&blob)?,
        annotations: attachment_annotations(descriptor),
    })
}

fn decode_solution_attachment(
    artifact: &ommx::artifact::LocalArtifactDyn,
    descriptor: &Descriptor,
) -> Result<crate::Solution> {
    let blob = attachment_blob(
        artifact,
        descriptor,
        Some(ommx::artifact::media_types::V1_SOLUTION_MEDIA_TYPE),
    )?;
    Ok(crate::Solution {
        inner: ommx::Solution::from_bytes(&blob)?,
        annotations: attachment_annotations(descriptor),
    })
}

fn decode_sample_set_attachment(
    artifact: &ommx::artifact::LocalArtifactDyn,
    descriptor: &Descriptor,
) -> Result<crate::SampleSet> {
    let blob = attachment_blob(
        artifact,
        descriptor,
        Some(ommx::artifact::media_types::V1_SAMPLE_SET_MEDIA_TYPE),
    )?;
    Ok(crate::SampleSet {
        inner: ommx::SampleSet::from_bytes(&blob)?,
        annotations: attachment_annotations(descriptor),
    })
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
/// to the parent experiment. On exception the run is abandoned. A run
/// becomes immutable once it is finished.
pub struct PyRun {
    run: Option<ommx::experiment::RunDyn>,
    store_trace: bool,
    context_entered: bool,
    context_active: bool,
    span_context_manager: Option<Py<PyAny>>,
    trace_result: Option<Py<PyAny>>,
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl PyRun {
    pub fn __enter__(slf: Bound<'_, Self>) -> PyResult<Py<PyRun>> {
        {
            let py = slf.py();
            let mut this = slf.borrow_mut();
            if this.context_entered {
                return Err(anyhow::anyhow!("Run context has already been entered").into());
            }
            let run_id = this.as_open()?.run_id()?;
            if this.store_trace {
                let tracing = py.import("ommx.tracing")?;
                let cm = tracing.getattr("capture_trace")?.call1(("ommx.run",))?;
                let result = cm.call_method0("__enter__")?;
                set_current_span_run_id(py, run_id)?;
                this.trace_result = Some(result.unbind());
                this.span_context_manager = Some(cm.unbind());
            } else {
                let cm = start_python_span(py, "ommx.run")?;
                set_current_span_run_id(py, run_id)?;
                this.span_context_manager = Some(cm);
            }
            this.context_entered = true;
            this.context_active = true;
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
        let close_result = close_python_context_manager(
            py,
            self.span_context_manager.take(),
            exc_type,
            exc_value,
            traceback,
        );
        self.context_active = false;

        if self.run.is_none() {
            self.trace_result = None;
            close_result?;
            return Ok(false);
        }

        if exc_type.is_some() {
            if let Some(run) = self.run.take() {
                run.abandon();
            }
            self.trace_result = None;
            close_result?;
            return Ok(false);
        }

        close_result?;
        self.store_trace_result(py)?;
        self.finish_inner()?;
        Ok(false)
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
            MediaType::Other(media_type.to_string()),
            bytes.as_bytes(),
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
            MediaType::Other("application/json".to_string()),
            blob,
        )
    }

    /// Attach an Instance in this run.
    ///
    /// This records an instance as a run-level attachment. Use `log_solve`
    /// when the instance is the input of a solver call and should be paired
    /// with the returned solution.
    pub fn log_instance(&mut self, name: &str, instance: &crate::Instance) -> Result<()> {
        self.ensure_store_trace_context_started()?;
        AttachmentLogger::log_instance(self.as_open_mut()?, name, &instance.inner)
    }

    /// Attach a ParametricInstance in this run.
    pub fn log_parametric_instance(
        &mut self,
        name: &str,
        pi: &crate::ParametricInstance,
    ) -> Result<()> {
        self.ensure_store_trace_context_started()?;
        AttachmentLogger::log_parametric_instance(self.as_open_mut()?, name, &pi.inner)
    }

    /// Attach a Solution in this run.
    ///
    /// This records a solution as a run-level attachment. Use `log_solve`
    /// when the solution is produced by a solver call and should be paired
    /// with the input instance.
    pub fn log_solution(&mut self, name: &str, solution: &crate::Solution) -> Result<()> {
        self.ensure_store_trace_context_started()?;
        AttachmentLogger::log_solution(self.as_open_mut()?, name, &solution.inner)
    }

    /// Attach a SampleSet in this run.
    pub fn log_sample_set(&mut self, name: &str, sample_set: &crate::SampleSet) -> Result<()> {
        self.ensure_store_trace_context_started()?;
        AttachmentLogger::log_sample_set(self.as_open_mut()?, name, &sample_set.inner)
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
        let adapter_name = adapter.name(py)?;
        let span = tracing::info_span!("ommx.solver.solve", adapter = %adapter_name);
        let _span_guard = span.enter();
        let adapter_options = dump_kwargs(py, kwargs)?;
        let solution = adapter.solve(py, instance, kwargs)?;
        let solve_id = self.as_open_mut()?.log_finished_solve_result(
            &instance.inner,
            &solution.inner,
            adapter_name,
            adapter_options,
        )?;
        tracing::info!(solve_id, "ommx.solve.recorded");
        Ok(solution)
    }

    /// Finish this run and append it to the parent Experiment.
    ///
    /// After this method returns, the run handle can no longer be used. The
    /// context manager calls this automatically on normal exit and abandons
    /// the run on exception.
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
        Ok(match &self.run {
            Some(run) => format!("Run(run_id={})", run.run_id()?),
            None => "Run(finished=True)".to_string(),
        })
    }
}

impl PyRun {
    fn ensure_store_trace_context_started(&self) -> Result<()> {
        if self.store_trace && self.run.is_some() && !self.context_active {
            anyhow::bail!("store_trace=True requires using Run as a context manager");
        }
        Ok(())
    }

    fn store_trace_result(&mut self, py: Python<'_>) -> Result<()> {
        if !self.store_trace {
            return Ok(());
        }
        let result = self
            .trace_result
            .take()
            .context("store_trace=True lost its TraceResult before Run exit")?;
        let payload: Vec<u8> = result.bind(py).call_method0("otlp_protobuf")?.extract()?;
        self.as_open_mut()?
            .store_trace_layer(ommx::experiment::Trace::from_bytes(payload))?;
        Ok(())
    }

    fn finish_inner(&mut self) -> Result<()> {
        let run = self
            .run
            .take()
            .ok_or_else(|| anyhow::anyhow!("Run has already been finished"))?;
        run.finish()
    }

    fn as_open(&self) -> Result<&ommx::experiment::RunDyn> {
        self.run
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Run has already been finished"))
    }

    fn as_open_mut(&mut self) -> Result<&mut ommx::experiment::RunDyn> {
        self.run
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Run has already been finished"))
    }
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
    fn solve(
        &self,
        py: Python<'_>,
        instance: &crate::Instance,
        kwargs: Option<&Bound<PyDict>>,
    ) -> Result<crate::Solution> {
        let adapter = self.0.bind(py);
        let adapter_instance = Py::new(py, instance.clone())?;
        let solution_object = adapter.call_method("solve", (adapter_instance,), kwargs)?;
        solution_object
            .extract::<crate::Solution>()
            .map_err(|_| anyhow::anyhow!("adapter.solve(...) must return ommx.v1.Solution"))
    }

    fn name(&self, py: Python<'_>) -> Result<String> {
        let adapter = self.0.bind(py);
        let module: String = adapter.module()?.extract()?;
        let qualname: String = adapter.qualname()?.extract()?;
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
/// Immutable view of a finished Run in a committed Experiment.
///
/// `SealedRun` exposes run-level attachments by name and the sequence of
/// `Solve` records created by `Run.log_solve`.
pub struct PySealedRun {
    run: ommx::experiment::SealedRunDyn,
    artifact: ommx::artifact::LocalArtifactDyn,
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
    /// Low-level descriptors for run-level attachments.
    ///
    /// Prefer `attachment_names`, `get_attachment`, or typed methods such as
    /// `get_json` and `get_instance` when working from Python. This descriptor
    /// view is kept for low-level Artifact inspection.
    pub fn attachments(&self) -> Result<Vec<PyDescriptor>> {
        Ok(self
            .run
            .attachments()?
            .into_iter()
            .map(PyDescriptor::from)
            .collect())
    }

    #[getter]
    /// Names of run-level attachments.
    pub fn attachment_names(&self) -> Result<Vec<String>> {
        Ok(self
            .run
            .attachments()?
            .into_iter()
            .filter_map(|descriptor| attachment_name(&descriptor).map(ToOwned::to_owned))
            .collect())
    }

    /// Read a run-level attachment by name.
    ///
    /// The returned Python object is decoded from the attachment media type.
    pub fn get_attachment<'py>(&self, py: Python<'py>, name: &str) -> Result<Bound<'py, PyAny>> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let descriptor = self.find_attachment(name)?;
        decode_attachment(py, &self.artifact, &descriptor)
    }

    /// Read a JSON run-level attachment by name.
    pub fn get_json<'py>(&self, py: Python<'py>, name: &str) -> Result<Bound<'py, PyAny>> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let descriptor = self.find_attachment(name)?;
        decode_json_attachment(py, &self.artifact, &descriptor)
    }

    /// Read an Instance run-level attachment by name.
    pub fn get_instance(&self, py: Python<'_>, name: &str) -> Result<crate::Instance> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let descriptor = self.find_attachment(name)?;
        decode_instance_attachment(&self.artifact, &descriptor)
    }

    /// Read a ParametricInstance run-level attachment by name.
    pub fn get_parametric_instance(
        &self,
        py: Python<'_>,
        name: &str,
    ) -> Result<crate::ParametricInstance> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let descriptor = self.find_attachment(name)?;
        decode_parametric_instance_attachment(&self.artifact, &descriptor)
    }

    /// Read a Solution run-level attachment by name.
    pub fn get_solution(&self, py: Python<'_>, name: &str) -> Result<crate::Solution> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let descriptor = self.find_attachment(name)?;
        decode_solution_attachment(&self.artifact, &descriptor)
    }

    /// Read a SampleSet run-level attachment by name.
    pub fn get_sample_set(&self, py: Python<'_>, name: &str) -> Result<crate::SampleSet> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let descriptor = self.find_attachment(name)?;
        decode_sample_set_attachment(&self.artifact, &descriptor)
    }

    /// Read raw bytes of a run-level attachment by name.
    pub fn get_blob<'py>(&self, py: Python<'py>, name: &str) -> Result<Bound<'py, PyBytes>> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let descriptor = self.find_attachment(name)?;
        Ok(PyBytes::new(py, &self.artifact.get_blob(&descriptor)?))
    }

    #[getter]
    /// Stored trace for this run, or `None` when this run was recorded without trace storage.
    pub fn trace<'py>(&self, py: Python<'py>) -> Result<Option<Bound<'py, PyAny>>> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let Some(descriptor) = self.run.trace_layer()? else {
            return Ok(None);
        };
        let blob = self.artifact.get_blob(&descriptor)?;
        let trace_result = py.import("ommx.tracing")?.getattr("TraceResult")?;
        Ok(Some(trace_result.call_method1(
            "from_otlp_protobuf",
            (PyBytes::new(py, &blob),),
        )?))
    }

    #[getter]
    /// Solve records logged in this run, ordered by `solve_id`.
    pub fn solves(&self) -> Vec<PySolve> {
        self.run.solves().iter().cloned().map(PySolve).collect()
    }

    pub fn __repr__(&self) -> String {
        format!(
            "SealedRun(run_id={}, attachments={}, solves={})",
            self.run_id(),
            self.run.attachment_count(),
            self.run.solves().len(),
        )
    }
}

impl PySealedRun {
    fn find_attachment(&self, name: &str) -> Result<Descriptor> {
        for descriptor in self.run.attachments()? {
            if attachment_name(&descriptor) == Some(name) {
                return Ok(Descriptor::from(descriptor));
            }
        }
        anyhow::bail!("Run {} attachment `{name}` not found", self.run.run_id())
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
        let descriptor = self.0.input()?;
        Ok(crate::Instance {
            inner: self.0.input_instance()?,
            annotations: descriptor
                .annotations()
                .as_ref()
                .cloned()
                .unwrap_or_default(),
        })
    }

    #[getter]
    /// Output `Solution` returned by the solver.
    pub fn output(&self) -> Result<crate::Solution> {
        let descriptor = self.0.output()?;
        Ok(crate::Solution {
            inner: self.0.output_solution()?,
            annotations: descriptor
                .annotations()
                .as_ref()
                .cloned()
                .unwrap_or_default(),
        })
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

    pub fn __repr__(&self) -> String {
        format!("Solve(solve_id={})", self.solve_id())
    }
}
