mod artifact;
mod builder;
mod descriptor;
mod evaluate;
mod message;

pub use artifact::*;
pub use builder::*;
pub use descriptor::*;
pub use evaluate::*;
pub use message::*;

use pyo3::prelude::*;

#[pymodule]
fn _ommx_rust(_py: Python, m: &Bound<PyModule>) -> PyResult<()> {
    pyo3_log::init();

    // OMMX Artifact
    m.add_class::<ArtifactArchive>()?;
    m.add_class::<ArtifactDir>()?;
    m.add_class::<ArtifactArchiveBuilder>()?;
    m.add_class::<ArtifactDirBuilder>()?;
    m.add_class::<PyDescriptor>()?;

    // OMMX Message
    m.add_class::<Linear>()?;
    m.add_class::<Quadratic>()?;
    m.add_class::<Polynomial>()?;
    m.add_class::<Function>()?;

    // Evaluate
    m.add_function(wrap_pyfunction!(evaluate_function, m)?)?;
    m.add_function(wrap_pyfunction!(evaluate_linear, m)?)?;
    m.add_function(wrap_pyfunction!(evaluate_quadratic, m)?)?;
    m.add_function(wrap_pyfunction!(evaluate_polynomial, m)?)?;
    m.add_function(wrap_pyfunction!(evaluate_constraint, m)?)?;
    m.add_function(wrap_pyfunction!(evaluate_instance, m)?)?;
    m.add_function(wrap_pyfunction!(used_decision_variable_ids, m)?)?;
    Ok(())
}
