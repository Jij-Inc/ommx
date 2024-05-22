mod artifact;
mod evaluate;
pub use artifact::*;
pub use evaluate::*;

use pyo3::prelude::*;

#[pymodule]
fn _ommx_rust(_py: Python, m: &Bound<PyModule>) -> PyResult<()> {
    m.add_class::<Artifact>()?;
    m.add_class::<Descriptor>()?;
    m.add_function(wrap_pyfunction!(evaluate_function, m)?)?;
    Ok(())
}
