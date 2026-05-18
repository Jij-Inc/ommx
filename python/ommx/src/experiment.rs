use anyhow::Result;
use pyo3::{prelude::*, types::PyDict};
use std::collections::BTreeMap;

use crate::pandas::{raw_entries_to_dataframe, PyDataFrame};
use crate::PyDescriptor;

#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[pyo3(module = "ommx._ommx_rust", name = "Experiment")]
pub struct PyExperiment(ommx::experiment::LoadedExperiment<'static>);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl PyExperiment {
    /// Load a committed Experiment Artifact from the local registry.
    #[staticmethod]
    pub fn load(py: Python<'_>, image_name: &str) -> Result<Self> {
        let _guard = crate::TRACING.attach_parent_context(py);
        let image_name = ommx::artifact::ImageRef::parse(image_name)?;
        let artifact = ommx::artifact::LocalArtifact::open(image_name)?;
        Ok(Self(ommx::experiment::LoadedExperiment::from_artifact(
            artifact,
        )?))
    }

    #[getter]
    pub fn image_name(&self) -> String {
        self.0.image_name().to_string()
    }

    #[getter]
    pub fn records(&self) -> Vec<PyExperimentRecord> {
        self.0
            .records()
            .iter()
            .cloned()
            .map(PyExperimentRecord)
            .collect()
    }

    /// Wide DataFrame of run parameters, indexed by `run_id`.
    pub fn run_parameters_df<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDataFrame>> {
        let mut rows = BTreeMap::new();
        for cell in self.0.run_parameter_cells() {
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
        raw_entries_to_dataframe(py, entries, "run_id")
    }

    pub fn __repr__(&self) -> String {
        format!("Experiment(image_name='{}')", self.image_name())
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
