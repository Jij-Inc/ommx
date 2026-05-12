#![allow(clippy::too_many_arguments)] // PyO3 functions often have many arguments, and it's not worth refactoring them to avoid this warning.

mod annotations;
mod artifact;
mod attached;
mod bound;
mod constraint;
#[cfg(feature = "remote-artifact")]
mod dataset;
mod decision_variable;
mod descriptor;
mod enums;
mod evaluated_constraint;
mod evaluated_decision_variable;
mod evaluated_named_function;
mod function;
mod indicator_constraint;
mod instance;
mod linear;
mod named_function;
mod one_hot_constraint;
mod pandas;
mod parameter;
mod parameters;
mod parametric_instance;
mod polynomial;
mod provenance;
mod quadratic;
mod random;
mod sample_set;
mod sampled_constraint;
mod sampled_decision_variable;
mod sampled_named_function;
mod samples;
mod solution;
mod sos1_constraint;
mod state;

pub use artifact::*;
// `attached.rs` is implementation detail — re-export only the host enum that
// the kind-specific binding files reference. The metadata-method macros are
// already exported via `#[macro_export]`.
pub use attached::ConstraintHost;
pub use bound::*;
pub use constraint::*;
#[cfg(feature = "remote-artifact")]
pub use dataset::*;
pub use decision_variable::*;
pub use descriptor::*;
pub use enums::*;
pub use evaluated_constraint::*;
pub use evaluated_decision_variable::*;
pub use evaluated_named_function::*;
pub use function::*;
pub use indicator_constraint::*;
pub use instance::*;
pub use linear::*;
pub use named_function::*;
pub use one_hot_constraint::*;
pub use parameter::*;
pub use parameters::*;
pub use parametric_instance::*;
pub use polynomial::*;
pub use provenance::*;
pub use quadratic::*;
pub use random::*;
pub use sample_set::*;
pub use sampled_constraint::*;
pub use sampled_decision_variable::*;
pub use sampled_named_function::*;
pub use samples::*;
pub use solution::*;
pub use sos1_constraint::*;
pub use state::*;

use pyo3::prelude::*;
use pyo3_stub_gen::runtime::PyModuleTypeAliasExt;

#[cfg(feature = "tracing-bridge")]
use pyo3_tracing_opentelemetry::TracingBridge;

/// No-op stand-in used when the `tracing-bridge` feature is disabled (e.g.
/// pyodide/wasm32-unknown-emscripten, where the opentelemetry crate transitively
/// pulls in `wasm-bindgen` and fails to load).
#[cfg(not(feature = "tracing-bridge"))]
pub(crate) struct TracingBridge;

#[cfg(not(feature = "tracing-bridge"))]
pub(crate) struct TracingBridgeGuard;

#[cfg(not(feature = "tracing-bridge"))]
impl TracingBridge {
    pub(crate) const fn new(_name: &'static str) -> Self {
        Self
    }

    pub(crate) fn attach_parent_context(&self, _py: Python) -> TracingBridgeGuard {
        TracingBridgeGuard
    }
}

/// Bridge Rust `tracing` spans/events to Python's OpenTelemetry SDK.
///
/// Entry points call `TRACING.attach_parent_context(py)` to initialize the
/// bridge (once per process) and adopt the Python-side trace context so Rust
/// spans appear as children of the current Python span.
pub(crate) const TRACING: TracingBridge = TracingBridge::new("ommx");

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
    // OMMX Artifact
    m.add_class::<PyDescriptor>()?;
    m.add_class::<PyArtifact>()?;
    m.add_class::<PyArchiveManifest>()?;
    m.add_class::<PyArtifactBuilder>()?;
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
    m.add_class::<AttachedDecisionVariable>()?;
    m.add_class::<Parameter>()?;
    m.add_class::<AdditionalCapability>()?;
    m.add_class::<Constraint>()?;
    m.add_class::<AttachedConstraint>()?;
    m.add_class::<IndicatorConstraint>()?;
    m.add_class::<AttachedIndicatorConstraint>()?;
    m.add_class::<RemovedIndicatorConstraint>()?;
    m.add_class::<OneHotConstraint>()?;
    m.add_class::<AttachedOneHotConstraint>()?;
    m.add_class::<RemovedOneHotConstraint>()?;
    m.add_class::<Sos1Constraint>()?;
    m.add_class::<AttachedSos1Constraint>()?;
    m.add_class::<RemovedSos1Constraint>()?;
    m.add_class::<NamedFunction>()?;
    m.add_class::<RemovedConstraint>()?;
    m.add_class::<Provenance>()?;
    m.add_class::<ProvenanceKind>()?;
    m.add_class::<ParametricInstance>()?;
    m.add_class::<Parameters>()?;
    m.add_class::<Solution>()?;
    m.add_class::<SampleSet>()?;
    m.add_class::<Samples>()?;
    m.add_class::<State>()?;
    m.add_type_alias::<ToState>()?;
    m.add_type_alias::<ScalarLike>()?;
    m.add_type_alias::<LinearLike>()?;
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
    "AttachedDecisionVariable",
    "Parameter",
    // Constraint capability
    "AdditionalCapability",
    // Constraint and named function
    "Constraint",
    "AttachedConstraint",
    "IndicatorConstraint",
    "AttachedIndicatorConstraint",
    "RemovedIndicatorConstraint",
    "OneHotConstraint",
    "AttachedOneHotConstraint",
    "RemovedOneHotConstraint",
    "Sos1Constraint",
    "AttachedSos1Constraint",
    "RemovedSos1Constraint",
    "RemovedConstraint",
    "Provenance",
    "ProvenanceKind",
    "NamedFunction",
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

pyo3_stub_gen::reexport_module_members!("ommx.artifact" from "ommx._ommx_rust";
    "Artifact",
    "ArchiveManifest",
    "ArtifactBuilder",
    "Descriptor",
    "get_local_registry_root",
    "set_local_registry_root",
    "get_image_dir",
    "get_images"
);

pyo3_stub_gen::define_stub_info_gatherer!(stub_info);
