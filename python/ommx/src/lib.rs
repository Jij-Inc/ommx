mod annotations;
mod artifact;
mod bound;
mod builder;
mod constraint;
mod constraint_hints;
#[cfg(feature = "remote-artifact")]
mod dataset;
mod decision_variable;
mod descriptor;
mod enums;
mod evaluated_constraint;
mod evaluated_decision_variable;
mod evaluated_named_function;
mod function;
mod instance;
mod linear;
mod named_function;
mod pandas;
mod parameter;
mod parameters;
mod parametric_instance;
mod polynomial;
mod quadratic;
mod random;
mod sample_set;
mod sampled_constraint;
mod sampled_decision_variable;
mod sampled_named_function;
mod samples;
mod solution;
mod state;

pub use artifact::*;
pub use bound::*;
pub use builder::*;
pub use constraint::*;
pub use constraint_hints::*;
#[cfg(feature = "remote-artifact")]
pub use dataset::*;
pub use decision_variable::*;
pub use descriptor::*;
pub use enums::*;
pub use evaluated_constraint::*;
pub use evaluated_decision_variable::*;
pub use evaluated_named_function::*;
pub use function::*;
pub use instance::*;
pub use linear::*;
pub use named_function::*;
pub use parameter::*;
pub use parameters::*;
pub use parametric_instance::*;
pub use polynomial::*;
pub use quadratic::*;
pub use random::*;
pub use sample_set::*;
pub use sampled_constraint::*;
pub use sampled_decision_variable::*;
pub use sampled_named_function::*;
pub use samples::*;
pub use solution::*;
pub use state::*;

use pyo3::prelude::*;
use pyo3_stub_gen::runtime::PyModuleTypeAliasExt;

#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
pub fn set_default_atol(value: f64) -> anyhow::Result<()> {
    ommx::ATol::set_default(value)
}

#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
pub fn get_default_atol() -> f64 {
    ommx::ATol::default().into_inner()
}

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
    m.add_function(wrap_pyfunction!(set_local_registry_root, m)?)?;
    m.add_function(wrap_pyfunction!(get_local_registry_root, m)?)?;
    m.add_function(wrap_pyfunction!(get_image_dir, m)?)?;
    m.add_function(wrap_pyfunction!(get_images, m)?)?;

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
    m.add_class::<Parameter>()?;
    m.add_class::<Constraint>()?;
    m.add_class::<NamedFunction>()?;
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
    m.add_type_alias::<ToState>()?;
    m.add_type_alias::<ToFunction>()?;
    m.add_type_alias::<ToSamples>()?;
    m.add_class::<EvaluatedDecisionVariable>()?;
    m.add_class::<EvaluatedConstraint>()?;
    m.add_class::<EvaluatedNamedFunction>()?;
    m.add_class::<SampledConstraint>()?;
    m.add_class::<SampledDecisionVariable>()?;
    m.add_class::<SampledNamedFunction>()?;

    // Enums
    m.add_class::<Sense>()?;
    m.add_class::<Equality>()?;
    m.add_class::<Kind>()?;
    m.add_class::<Optimality>()?;
    m.add_class::<Relaxation>()?;

    // Random
    m.add_class::<Rng>()?;

    // Dataset
    #[cfg(feature = "remote-artifact")]
    {
        m.add_function(wrap_pyfunction!(miplib2017_instance_annotations, m)?)?;
        m.add_function(wrap_pyfunction!(qplib_instance_annotations, m)?)?;
    }

    // ATol functions
    m.add_function(wrap_pyfunction!(set_default_atol, m)?)?;
    m.add_function(wrap_pyfunction!(get_default_atol, m)?)?;

    // Constraint ID management
    m.add_function(wrap_pyfunction!(next_constraint_id, m)?)?;
    m.add_function(wrap_pyfunction!(set_constraint_id_counter, m)?)?;
    m.add_function(wrap_pyfunction!(update_constraint_id_counter, m)?)?;
    m.add_function(wrap_pyfunction!(get_constraint_id_counter, m)?)?;

    Ok(())
}

pyo3_stub_gen::reexport_module_members!("ommx.v1" from "ommx._ommx_rust";
    // Enums
    "Sense",
    "Equality",
    "Kind",
    "Optimality",
    "Relaxation",
    // Core types
    "State",
    "Samples",
    "Bound",
    // Function types
    "Linear",
    "Quadratic",
    "Polynomial",
    "Function",
    // Decision variable and parameter
    "DecisionVariable",
    "Parameter",
    // Constraint and named function
    "Constraint",
    "RemovedConstraint",
    "NamedFunction",
    // Constraint hints
    "OneHot",
    "Sos1",
    "ConstraintHints",
    // Evaluated types
    "EvaluatedDecisionVariable",
    "EvaluatedConstraint",
    "EvaluatedNamedFunction",
    "SampledDecisionVariable",
    "SampledConstraint",
    "SampledNamedFunction",
    // Analysis
    "DecisionVariableAnalysis",
    // Top-level types
    "Instance",
    "ParametricInstance",
    "Solution",
    "SampleSet",
    // Utility
    "Rng",
    // Type aliases
    "ToState",
    "ToSamples"
);

pyo3_stub_gen::define_stub_info_gatherer!(stub_info);
