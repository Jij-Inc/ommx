use anyhow::Result;
use pyo3::prelude::*;

#[pyclass]
#[pyo3(module = "ommx._ommx_rust")]
pub struct Artifact(ommx::artifact::Artifact<ocipkg::image::OciArchive>);

#[pymethods]
impl Artifact {
    pub fn num_instances(&mut self) -> Result<usize> {
        Ok(self.0.get_instances()?.len())
    }
}

/// Formats the sum of two numbers as string.
#[pyfunction]
fn sum_as_string(a: usize, b: usize) -> PyResult<String> {
    Ok((a + b).to_string())
}

/// A Python module implemented in Rust.
#[pymodule]
fn _ommx_rust(_py: Python, m: Bound<PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(sum_as_string, &m)?)?;
    m.add_class::<Artifact>()?;
    Ok(())
}
