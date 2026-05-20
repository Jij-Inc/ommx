use anyhow::Result;
use oci_spec::image::MediaType;
use pyo3::{
    prelude::*,
    types::{PyBool, PyDict, PyFloat, PyInt, PyString},
};
use std::collections::BTreeMap;

use crate::pandas::{raw_entries_to_dataframe, PyDataFrame};
use crate::{PyArtifact, PyDescriptor};

#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[pyo3(module = "ommx._ommx_rust", name = "Experiment")]
pub struct PyExperiment {
    inner: ommx::experiment::ExperimentDyn,
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl PyExperiment {
    /// Start a new Experiment in the local registry.
    ///
    /// If `image_name` is omitted, OMMX generates an anonymous local
    /// Experiment name.
    #[new]
    #[pyo3(signature = (image_name = None))]
    pub fn new(image_name: Option<&str>) -> Result<Self> {
        Ok(Self {
            inner: ommx::experiment::ExperimentDyn::new(parse_name(image_name)?)?,
        })
    }

    /// Start a new Experiment backed by a temporary Local Registry.
    ///
    /// The temporary registry is kept alive by the returned Experiment
    /// and by Artifacts / loaded Experiments derived from it.
    #[staticmethod]
    #[pyo3(signature = (image_name = None))]
    pub fn with_temp_local_registry(image_name: Option<&str>) -> Result<Self> {
        Ok(Self {
            inner: ommx::experiment::ExperimentDyn::with_temp_local_registry(parse_name(
                image_name,
            )?)?,
        })
    }

    /// Load a committed Experiment Artifact from the local registry.
    #[staticmethod]
    pub fn load(py: Python<'_>, image_name: &str) -> Result<Self> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let image_name = ommx::artifact::ImageRef::parse(image_name)?;
        Ok(Self {
            inner: ommx::experiment::ExperimentDyn::load(image_name)?,
        })
    }

    /// Interpret an already-open Artifact as a committed Experiment.
    #[staticmethod]
    pub fn from_artifact(artifact: &PyArtifact) -> Result<Self> {
        Ok(Self {
            inner: ommx::experiment::ExperimentDyn::from_artifact(artifact.inner().clone())?,
        })
    }

    pub fn __enter__(slf: Bound<'_, Self>) -> PyResult<Py<PyExperiment>> {
        Ok(slf.unbind())
    }

    #[pyo3(signature = (exc_type = None, _exc_value = None, _traceback = None))]
    pub fn __exit__(
        &mut self,
        py: Python<'_>,
        exc_type: Option<&Bound<'_, PyAny>>,
        _exc_value: Option<&Bound<'_, PyAny>>,
        _traceback: Option<&Bound<'_, PyAny>>,
    ) -> Result<bool> {
        if exc_type.is_none() && self.inner.is_unsealed() {
            self.commit(py)?;
        }
        Ok(false)
    }

    #[getter]
    pub fn image_name(&self) -> Result<String> {
        Ok(self.inner.image_name()?.to_string())
    }

    #[getter]
    pub fn records(&self) -> Result<Vec<PyExperimentRecord>> {
        Ok(self
            .inner
            .records()?
            .into_iter()
            .map(PyExperimentRecord)
            .collect())
    }

    #[getter]
    pub fn artifact(&self) -> Result<PyArtifact> {
        Ok(PyArtifact::new(self.inner.artifact()?))
    }

    /// Start a new Run in this unsealed Experiment.
    pub fn run(&self) -> Result<PyRun> {
        Ok(PyRun {
            run: Some(self.inner.run()?),
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
        self.inner.log_record(
            name,
            MediaType::Other(media_type.to_string()),
            bytes.as_bytes(),
        )
    }

    /// Record a JSON-serialisable value in the experiment space.
    pub fn log_json(&mut self, py: Python<'_>, name: &str, value: &Bound<PyAny>) -> Result<()> {
        let json = py.import("json")?;
        let blob: String = json.call_method1("dumps", (value,))?.extract()?;
        self.inner
            .log_record(name, MediaType::Other("application/json".to_string()), blob)
    }

    /// Record an Instance in the experiment space.
    pub fn log_instance(&mut self, name: &str, instance: &crate::Instance) -> Result<()> {
        self.inner.log_instance(name, &instance.inner)
    }

    /// Record a Solution in the experiment space.
    pub fn log_solution(&mut self, name: &str, solution: &crate::Solution) -> Result<()> {
        self.inner.log_solution(name, &solution.inner)
    }

    /// Record a SampleSet in the experiment space.
    pub fn log_sample_set(&mut self, name: &str, sample_set: &crate::SampleSet) -> Result<()> {
        self.inner.log_sample_set(name, &sample_set.inner)
    }

    /// Commit this unsealed Experiment into the local registry.
    pub fn commit(&mut self, py: Python<'_>) -> Result<PyArtifact> {
        let _guard = crate::TRACING.attach_parent_context(py);
        Ok(PyArtifact::new(self.inner.commit()?))
    }

    /// Wide DataFrame of run parameters, indexed by `run_id`.
    pub fn run_parameters_df<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyDataFrame>> {
        let mut rows = BTreeMap::new();
        for cell in self.inner.run_parameter_cells()? {
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
        Ok(format!(
            "Experiment(image_name='{}', state='{}', open_runs={})",
            self.image_name()?,
            self.inner.state_name(),
            self.inner.open_run_count(),
        ))
    }
}

#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[pyo3(module = "ommx._ommx_rust", name = "Run")]
pub struct PyRun {
    run: Option<ommx::experiment::RunDyn>,
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl PyRun {
    pub fn __enter__(slf: Bound<'_, Self>) -> PyResult<Py<PyRun>> {
        Ok(slf.unbind())
    }

    #[pyo3(signature = (exc_type = None, _exc_value = None, _traceback = None))]
    pub fn __exit__(
        &mut self,
        _py: Python<'_>,
        exc_type: Option<&Bound<'_, PyAny>>,
        _exc_value: Option<&Bound<'_, PyAny>>,
        _traceback: Option<&Bound<'_, PyAny>>,
    ) -> Result<bool> {
        if self.run.is_none() {
            return Ok(false);
        }
        if exc_type.is_none() {
            self.finish()?;
        } else {
            if let Some(run) = self.run.take() {
                run.abandon();
            }
        }
        Ok(false)
    }

    #[getter]
    pub fn run_id(&self) -> Result<u64> {
        self.as_open()?.run_id()
    }

    /// Record a scalar parameter for this run.
    pub fn log_parameter(&mut self, name: &str, value: ParameterValueInput) -> Result<()> {
        self.as_open_mut()?.log_parameter(name, value.0)
    }

    /// Record arbitrary bytes with an explicit OCI media type in this run.
    pub fn log_record(
        &mut self,
        name: &str,
        media_type: &str,
        bytes: &Bound<pyo3::types::PyBytes>,
    ) -> Result<()> {
        self.as_open_mut()?.log_record(
            name,
            MediaType::Other(media_type.to_string()),
            bytes.as_bytes(),
        )
    }

    /// Record a JSON-serialisable value in this run.
    pub fn log_json(&mut self, py: Python<'_>, name: &str, value: &Bound<PyAny>) -> Result<()> {
        let json = py.import("json")?;
        let blob: String = json.call_method1("dumps", (value,))?.extract()?;
        self.as_open_mut()?
            .log_record(name, MediaType::Other("application/json".to_string()), blob)
    }

    /// Record an Instance in this run.
    pub fn log_instance(&mut self, name: &str, instance: &crate::Instance) -> Result<()> {
        self.as_open_mut()?.log_instance(name, &instance.inner)
    }

    /// Record a Solution in this run.
    pub fn log_solution(&mut self, name: &str, solution: &crate::Solution) -> Result<()> {
        self.as_open_mut()?.log_solution(name, &solution.inner)
    }

    /// Record a SampleSet in this run.
    pub fn log_sample_set(&mut self, name: &str, sample_set: &crate::SampleSet) -> Result<()> {
        self.as_open_mut()?.log_sample_set(name, &sample_set.inner)
    }

    /// Finish this run and append it to the parent Experiment.
    pub fn finish(&mut self) -> Result<()> {
        let run = self
            .run
            .take()
            .ok_or_else(|| anyhow::anyhow!("Run has already been finished"))?;
        run.finish()
    }

    pub fn __repr__(&self) -> Result<String> {
        Ok(match &self.run {
            Some(run) => format!("Run(run_id={})", run.run_id()?),
            None => "Run(finished=True)".to_string(),
        })
    }
}

impl PyRun {
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
