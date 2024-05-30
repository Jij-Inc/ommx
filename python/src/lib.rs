mod artifact;
mod descriptor;
mod evaluate;
pub use artifact::*;
pub use descriptor::*;
pub use evaluate::*;

use pyo3::prelude::*;

#[pymodule]
fn _ommx_rust(_py: Python, m: &Bound<PyModule>) -> PyResult<()> {
    m.add_class::<ArtifactArchive>()?;
    m.add_class::<PyDescriptor>()?;
    m.add_function(wrap_pyfunction!(evaluate_function, m)?)?;
    m.add_function(wrap_pyfunction!(evaluate_linear, m)?)?;
    m.add_function(wrap_pyfunction!(evaluate_quadratic, m)?)?;
    m.add_function(wrap_pyfunction!(evaluate_polynomial, m)?)?;
    m.add_function(wrap_pyfunction!(evaluate_constraint, m)?)?;
    m.add_function(wrap_pyfunction!(evaluate_instance, m)?)?;
    Ok(())
}
