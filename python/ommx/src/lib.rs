mod artifact;
mod bound;
mod builder;
mod constraint;
mod constraint_hints;
mod dataset;
mod decision_variable;
mod descriptor;
mod enums;
mod evaluated_constraint;
mod evaluated_decision_variable;
mod function;
mod instance;
mod linear;
mod mps;
mod polynomial;
mod qplib;
mod quadratic;
mod random;
mod sample_set;
mod sampled_constraint;
mod sampled_decision_variable;
mod sampled_values;
mod samples;
mod solution;
mod state;

pub use artifact::*;
pub use bound::*;
pub use builder::*;
pub use constraint::*;
pub use constraint_hints::*;
pub use dataset::*;
pub use decision_variable::*;
pub use descriptor::*;
pub use enums::*;
pub use evaluated_constraint::*;
pub use evaluated_decision_variable::*;
pub use function::*;
pub use instance::*;
pub use linear::*;
pub use mps::*;
pub use polynomial::*;
pub use qplib::*;
pub use quadratic::*;
pub use random::*;
pub use sample_set::*;
pub use sampled_constraint::*;
pub use sampled_decision_variable::*;
pub use sampled_values::*;
pub use samples::*;
pub use solution::*;
pub use state::*;

use pyo3::prelude::*;

/// We need `gil_used = false` to allow Python 3.13t
/// See <https://pyo3.rs/main/free-threading#supporting-free-threaded-python-with-pyo3>.
#[pymodule(gil_used = false)]
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
    m.add_class::<VariableBound>()?;
    m.add_class::<Instance>()?;
    m.add_class::<InstanceDescription>()?;
    m.add_class::<DecisionVariableAnalysis>()?;
    m.add_class::<DecisionVariable>()?;
    m.add_class::<Constraint>()?;
    m.add_class::<RemovedConstraint>()?;
    m.add_class::<OneHot>()?;
    m.add_class::<Sos1>()?;
    m.add_class::<ConstraintHints>()?;
    m.add_class::<ParametricInstance>()?;
    m.add_class::<Parameters>()?;
    m.add_class::<Solution>()?;
    m.add_class::<SampleSet>()?;
    m.add_class::<Samples>()?;
    m.add_class::<State>()?;
    m.add_class::<EvaluatedDecisionVariable>()?;
    m.add_class::<EvaluatedConstraint>()?;
    m.add_class::<SampledConstraint>()?;
    m.add_class::<SampledDecisionVariable>()?;
    m.add_class::<SampledValues>()?;
    m.add_class::<SampledValuesEntry>()?;

    // Enums
    m.add_class::<Sense>()?;
    m.add_class::<Equality>()?;
    m.add_class::<Kind>()?;
    m.add_class::<Optimality>()?;
    m.add_class::<Relaxation>()?;

    // Random
    m.add_class::<Rng>()?;

    // MPS
    m.add_function(wrap_pyfunction!(load_mps_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(write_mps_file, m)?)?;

    // Qplib
    m.add_function(wrap_pyfunction!(load_qplib_bytes, m)?)?;

    // Dataset
    m.add_function(wrap_pyfunction!(miplib2017_instance_annotations, m)?)?;
    Ok(())
}

#[cfg(feature = "stub_gen")]
pyo3_stub_gen::define_stub_info_gatherer!(stub_info);
