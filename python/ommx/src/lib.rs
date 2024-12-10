mod artifact;
mod builder;
mod dataset;
mod descriptor;
mod evaluate;
mod instance;
mod message;
mod mps;

pub use artifact::*;
pub use builder::*;
pub use dataset::*;
pub use descriptor::*;
pub use evaluate::*;
pub use instance::*;
pub use message::*;
pub use mps::*;

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
    m.add_function(wrap_pyfunction!(partial_evaluate_linear, m)?)?;
    m.add_function(wrap_pyfunction!(partial_evaluate_quadratic, m)?)?;
    m.add_function(wrap_pyfunction!(partial_evaluate_polynomial, m)?)?;
    m.add_function(wrap_pyfunction!(partial_evaluate_function, m)?)?;
    m.add_function(wrap_pyfunction!(partial_evaluate_constraint, m)?)?;
    m.add_function(wrap_pyfunction!(partial_evaluate_instance, m)?)?;
    m.add_function(wrap_pyfunction!(used_decision_variable_ids, m)?)?;

    // Instance
    m.add_function(wrap_pyfunction!(instance_to_pubo, m)?)?;
    m.add_function(wrap_pyfunction!(instance_to_qubo, m)?)?;

    // MPS
    m.add_function(wrap_pyfunction!(load_mps_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(write_mps_file, m)?)?;

    // Dataset
    m.add_function(wrap_pyfunction!(miplib2017_instance_annotations, m)?)?;
    Ok(())
}

#[cfg(feature = "stub_gen")]
pyo3_stub_gen::define_stub_info_gatherer!(stub_info);
