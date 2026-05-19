use anyhow::{bail, Result};
use oci_spec::image::MediaType;
use pyo3::{exceptions::PyRuntimeError, prelude::*, types::PyDict};
use std::collections::BTreeMap;

use crate::pandas::{raw_entries_to_dataframe, PyDataFrame};
use crate::{PyArtifact, PyDescriptor};

#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[pyo3(module = "ommx._ommx_rust", name = "Experiment")]
pub struct PyExperiment {
    inner: PyExperimentInner,
}

enum PyExperimentInner {
    Unsealed {
        experiment: Option<Box<ommx::experiment::Experiment<'static>>>,
        open_runs: usize,
    },
    Loaded(ommx::experiment::LoadedExperiment<'static>),
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl PyExperiment {
    /// Start a new Experiment in the local registry.
    ///
    /// If `image_name` is omitted, OMMX generates an anonymous local
    /// Experiment name.
    #[staticmethod]
    #[pyo3(signature = (image_name = None))]
    pub fn new(image_name: Option<&str>) -> Result<Self> {
        let name = match image_name {
            Some(image_name) => {
                ommx::experiment::Name::Named(ommx::artifact::ImageRef::parse(image_name)?)
            }
            None => ommx::experiment::Name::Anonymous,
        };
        Ok(Self {
            inner: PyExperimentInner::Unsealed {
                experiment: Some(Box::new(ommx::experiment::Experiment::new(name)?)),
                open_runs: 0,
            },
        })
    }

    /// Run a callback with a new Experiment backed by a temporary Local
    /// Registry. The temporary registry is deleted when the callback
    /// returns, so registry-backed handles created inside the callback
    /// must not be used afterwards.
    #[staticmethod]
    #[pyo3(signature = (callback, image_name = None))]
    pub fn with_temp_local_registry(
        py: Python<'_>,
        callback: &Bound<'_, PyAny>,
        image_name: Option<&str>,
    ) -> PyResult<Py<PyAny>> {
        let name = parse_name(image_name).map_err(to_py_runtime_error)?;
        let temp = ommx::artifact::local_registry::TempLocalRegistry::new()
            .map_err(to_py_runtime_error)?;
        let experiment = ommx::experiment::Experiment::with_registry(temp.registry(), name)
            .map_err(to_py_runtime_error)?;

        // The Python object is usable only while this function is
        // executing. It is invalidated before the temporary registry
        // is dropped, so escaped Experiment/Run handles fail before
        // touching registry-backed Rust state.
        let experiment = unsafe {
            std::mem::transmute::<
                ommx::experiment::Experiment<'_>,
                ommx::experiment::Experiment<'static>,
            >(experiment)
        };
        let py_experiment = Py::new(
            py,
            Self {
                inner: PyExperimentInner::Unsealed {
                    experiment: Some(Box::new(experiment)),
                    open_runs: 0,
                },
            },
        )?;

        let result = callback.call1((py_experiment.clone_ref(py),));
        let invalidation = Self::invalidate_temp_experiment(py, &py_experiment);
        match (result, invalidation) {
            (Ok(value), Ok(())) => Ok(value.unbind()),
            (Err(err), _) => Err(err),
            (Ok(_), Err(err)) => Err(err),
        }
    }

    /// Load a committed Experiment Artifact from the local registry.
    #[staticmethod]
    pub fn load(py: Python<'_>, image_name: &str) -> Result<Self> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let image_name = ommx::artifact::ImageRef::parse(image_name)?;
        let artifact = ommx::artifact::LocalArtifact::open(image_name)?;
        Ok(Self {
            inner: PyExperimentInner::Loaded(ommx::experiment::LoadedExperiment::from_artifact(
                artifact,
            )?),
        })
    }

    /// Interpret an already-open Artifact as a committed Experiment.
    #[staticmethod]
    pub fn from_artifact(artifact: &PyArtifact) -> Result<Self> {
        Ok(Self {
            inner: PyExperimentInner::Loaded(ommx::experiment::LoadedExperiment::from_artifact(
                artifact.0.clone(),
            )?),
        })
    }

    #[getter]
    pub fn image_name(&self) -> Result<String> {
        match &self.inner {
            PyExperimentInner::Unsealed { experiment, .. } => Ok(experiment
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Experiment has already been committed"))?
                .image_name()
                .to_string()),
            PyExperimentInner::Loaded(loaded) => Ok(loaded.image_name().to_string()),
        }
    }

    #[getter]
    pub fn records(&self) -> Result<Vec<PyExperimentRecord>> {
        let loaded = self.as_loaded()?;
        Ok(loaded
            .records()
            .iter()
            .cloned()
            .map(PyExperimentRecord)
            .collect())
    }

    /// Start a new Run in this unsealed Experiment.
    pub fn run(slf: Bound<'_, Self>) -> Result<PyRun> {
        let run = {
            let mut experiment = slf.borrow_mut();
            let (rust_experiment, open_runs) = experiment.as_unsealed_mut()?;
            let run = rust_experiment.run()?;
            *open_runs += 1;

            // `Run` borrows `rust_experiment`, whose address is stable
            // while it is inside the Box. `PyRun` keeps the parent Python
            // object alive and `Experiment.commit()` is blocked while any
            // run is open, so the borrowed Experiment cannot be moved or
            // consumed before the run finishes.
            unsafe {
                std::mem::transmute::<
                    ommx::experiment::Run<'_, 'static>,
                    ommx::experiment::Run<'static, 'static>,
                >(run)
            }
        };

        Ok(PyRun {
            parent: slf.unbind(),
            run: Some(run),
            closed: false,
        })
    }

    /// Record arbitrary bytes with an explicit OCI media type in the
    /// experiment space.
    pub fn log_record(
        &mut self,
        name: &str,
        media_type: &str,
        bytes: &Bound<pyo3::types::PyBytes>,
    ) -> Result<()> {
        self.as_unsealed()?.log_record(
            name,
            MediaType::Other(media_type.to_string()),
            bytes.as_bytes(),
        )
    }

    /// Record a JSON-serialisable value in the experiment space.
    pub fn log_json(&mut self, py: Python<'_>, name: &str, value: &Bound<PyAny>) -> Result<()> {
        let json = py.import("json")?;
        let blob: String = json.call_method1("dumps", (value,))?.extract()?;
        self.as_unsealed()?
            .log_record(name, MediaType::Other("application/json".to_string()), blob)
    }

    /// Record an Instance in the experiment space.
    pub fn log_instance(&mut self, name: &str, instance: &crate::Instance) -> Result<()> {
        self.as_unsealed()?.log_instance(name, &instance.inner)
    }

    /// Record a Solution in the experiment space.
    pub fn log_solution(&mut self, name: &str, solution: &crate::Solution) -> Result<()> {
        self.as_unsealed()?.log_solution(name, &solution.inner)
    }

    /// Record a SampleSet in the experiment space.
    pub fn log_sample_set(&mut self, name: &str, sample_set: &crate::SampleSet) -> Result<()> {
        self.as_unsealed()?.log_sample_set(name, &sample_set.inner)
    }

    /// Commit this unsealed Experiment into the local registry.
    pub fn commit(&mut self, py: Python<'_>) -> Result<PyArtifact> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let PyExperimentInner::Unsealed {
            experiment,
            open_runs,
        } = &mut self.inner
        else {
            bail!("Loaded Experiment is already committed");
        };
        if *open_runs != 0 {
            bail!("Cannot commit Experiment while {open_runs} Run handle(s) are still open");
        }
        let experiment = experiment
            .take()
            .ok_or_else(|| anyhow::anyhow!("Experiment has already been committed"))?;
        Ok(PyArtifact(experiment.commit()?.into_artifact()))
    }

    /// Wide DataFrame of run parameters, indexed by `run_id`.
    pub fn run_parameters_df<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyDataFrame>> {
        let loaded = self.as_loaded()?;
        let mut rows = BTreeMap::new();
        for cell in loaded.run_parameter_cells() {
            let row = rows.entry(cell.run_id).or_insert_with(|| {
                let dict = PyDict::new(py);
                dict.set_item("run_id", cell.run_id)
                    .expect("setting run_id in a new Python dict cannot fail");
                dict
            });
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
        Ok(format!("Experiment(image_name='{}')", self.image_name()?))
    }
}

impl PyExperiment {
    fn invalidate_temp_experiment(py: Python<'_>, experiment: &Py<Self>) -> PyResult<()> {
        let mut experiment = experiment.borrow_mut(py);
        let PyExperimentInner::Unsealed {
            experiment: rust_experiment,
            open_runs,
        } = &mut experiment.inner
        else {
            return Ok(());
        };
        rust_experiment.take();
        if *open_runs != 0 {
            *open_runs = 0;
            return Err(PyRuntimeError::new_err(
                "All Run handles created in with_temp_local_registry() must be finished before the callback returns",
            ));
        }
        Ok(())
    }

    fn as_loaded(&self) -> Result<&ommx::experiment::LoadedExperiment<'static>> {
        match &self.inner {
            PyExperimentInner::Loaded(loaded) => Ok(loaded),
            PyExperimentInner::Unsealed { .. } => {
                bail!("Experiment must be committed and loaded before using this view")
            }
        }
    }

    fn as_unsealed(&self) -> Result<&ommx::experiment::Experiment<'static>> {
        match &self.inner {
            PyExperimentInner::Unsealed { experiment, .. } => experiment
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("Experiment has already been committed")),
            PyExperimentInner::Loaded(_) => bail!("Loaded Experiment is read-only"),
        }
    }

    fn as_unsealed_mut(&mut self) -> Result<(&ommx::experiment::Experiment<'static>, &mut usize)> {
        match &mut self.inner {
            PyExperimentInner::Unsealed {
                experiment,
                open_runs,
            } => Ok((
                experiment
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("Experiment has already been committed"))?,
                open_runs,
            )),
            PyExperimentInner::Loaded(_) => bail!("Loaded Experiment is read-only"),
        }
    }
}

#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[pyo3(module = "ommx._ommx_rust", name = "Run")]
pub struct PyRun {
    parent: Py<PyExperiment>,
    run: Option<ommx::experiment::Run<'static, 'static>>,
    closed: bool,
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl PyRun {
    #[getter]
    pub fn run_id(&self, py: Python<'_>) -> Result<u64> {
        self.ensure_parent_active(py)?;
        Ok(self
            .run
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Run has already been finished"))?
            .run_id())
    }

    /// Record a scalar parameter for this run.
    pub fn log_parameter(
        &mut self,
        py: Python<'_>,
        name: &str,
        value: ParameterValueInput,
    ) -> Result<()> {
        self.as_open_mut(py)?.log_parameter(name, value.0)
    }

    /// Record arbitrary bytes with an explicit OCI media type in this run.
    pub fn log_record(
        &mut self,
        py: Python<'_>,
        name: &str,
        media_type: &str,
        bytes: &Bound<pyo3::types::PyBytes>,
    ) -> Result<()> {
        self.as_open_mut(py)?.log_record(
            name,
            MediaType::Other(media_type.to_string()),
            bytes.as_bytes(),
        )
    }

    /// Record a JSON-serialisable value in this run.
    pub fn log_json(&mut self, py: Python<'_>, name: &str, value: &Bound<PyAny>) -> Result<()> {
        let json = py.import("json")?;
        let blob: String = json.call_method1("dumps", (value,))?.extract()?;
        self.as_open_mut(py)?.log_record(
            name,
            MediaType::Other("application/json".to_string()),
            blob,
        )
    }

    /// Record an Instance in this run.
    pub fn log_instance(
        &mut self,
        py: Python<'_>,
        name: &str,
        instance: &crate::Instance,
    ) -> Result<()> {
        self.as_open_mut(py)?.log_instance(name, &instance.inner)
    }

    /// Record a Solution in this run.
    pub fn log_solution(
        &mut self,
        py: Python<'_>,
        name: &str,
        solution: &crate::Solution,
    ) -> Result<()> {
        self.as_open_mut(py)?.log_solution(name, &solution.inner)
    }

    /// Record a SampleSet in this run.
    pub fn log_sample_set(
        &mut self,
        py: Python<'_>,
        name: &str,
        sample_set: &crate::SampleSet,
    ) -> Result<()> {
        self.as_open_mut(py)?
            .log_sample_set(name, &sample_set.inner)
    }

    /// Finish this run and append it to the parent Experiment.
    pub fn finish(&mut self, py: Python<'_>) -> Result<()> {
        self.ensure_parent_active(py)?;
        let run = self
            .run
            .take()
            .ok_or_else(|| anyhow::anyhow!("Run has already been finished"))?;
        let result = run.finish();
        self.closed = true;
        self.decrement_open_runs(py)?;
        result
    }

    pub fn __repr__(&self, py: Python<'_>) -> Result<String> {
        self.ensure_parent_active(py)?;
        Ok(match &self.run {
            Some(run) => format!("Run(run_id={})", run.run_id()),
            None => "Run(finished=True)".to_string(),
        })
    }
}

impl PyRun {
    fn ensure_parent_active(&self, py: Python<'_>) -> Result<()> {
        let parent = self.parent.bind(py).borrow();
        match &parent.inner {
            PyExperimentInner::Unsealed {
                experiment: Some(_),
                ..
            } => Ok(()),
            PyExperimentInner::Unsealed {
                experiment: None, ..
            } => bail!("Parent Experiment is no longer active"),
            PyExperimentInner::Loaded(_) => bail!("Parent Experiment is no longer unsealed"),
        }
    }

    fn as_open_mut(
        &mut self,
        py: Python<'_>,
    ) -> Result<&mut ommx::experiment::Run<'static, 'static>> {
        self.ensure_parent_active(py)?;
        self.run
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Run has already been finished"))
    }

    fn decrement_open_runs(&mut self, py: Python<'_>) -> Result<()> {
        let mut parent = self.parent.bind(py).borrow_mut();
        let PyExperimentInner::Unsealed { open_runs, .. } = &mut parent.inner else {
            bail!("Parent Experiment is no longer unsealed");
        };
        *open_runs = open_runs.saturating_sub(1);
        Ok(())
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

fn to_py_runtime_error(error: anyhow::Error) -> PyErr {
    PyRuntimeError::new_err(error.to_string())
}

impl Drop for PyRun {
    fn drop(&mut self) {
        if self.closed || self.run.is_none() {
            return;
        }
        Python::attach(|py| {
            let _ = self.decrement_open_runs(py);
        });
    }
}

pub struct ParameterValueInput(ommx::experiment::ParameterValue);

impl<'py> FromPyObject<'_, 'py> for ParameterValueInput {
    type Error = PyErr;

    fn extract(ob: pyo3::Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
        if let Ok(value) = ob.extract::<bool>() {
            return Ok(Self(ommx::experiment::ParameterValue::Bool(value)));
        }
        if let Ok(value) = ob.extract::<i64>() {
            return Ok(Self(ommx::experiment::ParameterValue::Int(value)));
        }
        if let Ok(value) = ob.extract::<f64>() {
            return Ok(Self(ommx::experiment::ParameterValue::Float(value)));
        }
        if let Ok(value) = ob.extract::<String>() {
            return Ok(Self(ommx::experiment::ParameterValue::String(value)));
        }
        Err(pyo3::exceptions::PyTypeError::new_err(
            "Run parameter value must be bool, int, float, or str",
        ))
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
#[pyo3(module = "ommx._ommx_rust", name = "ExperimentRecord")]
#[derive(Clone)]
pub struct PyExperimentRecord(ommx::experiment::ExperimentRecord);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl PyExperimentRecord {
    #[getter]
    pub fn space(&self) -> &'static str {
        self.0.space.as_str()
    }

    #[getter]
    pub fn run_id(&self) -> Option<u64> {
        self.0.run_id
    }

    #[getter]
    pub fn name(&self) -> &str {
        &self.0.name
    }

    #[getter]
    pub fn media_type(&self) -> &str {
        &self.0.media_type
    }

    #[getter]
    pub fn descriptor(&self) -> PyDescriptor {
        PyDescriptor::from(self.0.descriptor.clone())
    }

    pub fn __repr__(&self) -> String {
        format!(
            "ExperimentRecord(space='{}', run_id={:?}, name='{}', media_type='{}')",
            self.space(),
            self.run_id(),
            self.name(),
            self.media_type(),
        )
    }
}
