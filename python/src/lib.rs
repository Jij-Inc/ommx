use anyhow::Result;
use ocipkg::{image::OciArchive, oci_spec::image::Descriptor as RawDescriptor};
use ommx::{
    artifact::media_types,
    v1::{Function, State},
    Evaluate, Message,
};
use pyo3::{prelude::*, types::PyBytes};
use std::{
    collections::{BTreeSet, HashMap},
    path::PathBuf,
};

#[pyclass]
#[pyo3(module = "ommx._ommx_rust")]
pub struct Descriptor(RawDescriptor);

#[pymethods]
impl Descriptor {
    #[getter]
    pub fn digest(&self) -> &str {
        self.0.digest()
    }

    #[getter]
    pub fn size(&self) -> i64 {
        self.0.size()
    }

    #[getter]
    pub fn annotations(&self) -> HashMap<String, String> {
        if let Some(annotations) = self.0.annotations() {
            annotations.clone()
        } else {
            HashMap::new()
        }
    }
}

#[pyclass]
#[pyo3(module = "ommx._ommx_rust")]
pub struct Artifact(ommx::artifact::Artifact<OciArchive>);

#[pymethods]
impl Artifact {
    #[staticmethod]
    pub fn from_oci_archive(path: PathBuf) -> Result<Self> {
        let artifact = ommx::artifact::Artifact::from_oci_archive(&path)?;
        Ok(Self(artifact))
    }

    #[getter]
    pub fn instance_descriptors(&mut self) -> Result<Vec<Descriptor>> {
        self.0
            .get_layer_descriptors(&media_types::v1_instance())
            .map(|descs| descs.into_iter().map(Descriptor).collect())
    }
}

#[pyfunction]
pub fn evaluate_function<'py>(
    function: &Bound<'py, PyBytes>,
    state: &Bound<'py, PyBytes>,
) -> Result<(f64, BTreeSet<u64>)> {
    let state = State::decode(state.as_bytes())?;
    let function = Function::decode(function.as_bytes())?;
    function.evaluate(&state)
}

#[pymodule]
fn _ommx_rust(_py: Python, m: &Bound<PyModule>) -> PyResult<()> {
    m.add_class::<Artifact>()?;
    m.add_class::<Descriptor>()?;
    m.add_function(wrap_pyfunction!(evaluate_function, m)?)?;
    Ok(())
}
