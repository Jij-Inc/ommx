//! Downstream PyO3 extension used to exercise the bridge across a DSO boundary.

mod transfer;

use ommx::{Evaluate as _, ParametricInstance};
use ommx_pyo3_bridge::{
    BridgeError, PyConstraint, PyDecisionVariable, PyFunction, PyInstance, PyParametricInstance,
    PySampleSet, PySolution,
};
use pyo3::{prelude::*, types::PyType};
use std::collections::{BTreeMap, HashMap};

fn v2_target(py: Python<'_>) -> PyResult<ommx_pyo3_bridge::Target<ommx_pyo3_bridge::ProtobufV2>> {
    ommx_pyo3_bridge::resolve_target::<ommx_pyo3_bridge::ProtobufV2>(py)?.ok_or_else(|| {
        BridgeError::new_err(
            py,
            "The loaded OMMX Python SDK does not support the protobuf v2 transfer protocol",
        )
    })
}

#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
fn bridge_error_type(py: Python<'_>) -> PyResult<Py<PyType>> {
    BridgeError::type_object(py).map(Bound::unbind)
}

fn component_function() -> ommx::Function {
    let linear = (ommx::linear!(7) + ommx::coeff!(-3.0))
        .expect("the fixture uses finite, non-zero coefficients");
    ommx::Function::from(linear)
}

fn composed_component_function() -> ommx::Function {
    let base = component_function();
    let two = ommx::Function::from(ommx::coeff!(2.0));
    let numerator = base.clone().abs().powi(2).min(two.clone());
    let denominator = (base.signum() + two).expect("the fixture addition is finite");
    (numerator / denominator).expect("the fixture division constructs a formal expression")
}

fn modeling_label(name: &str) -> ommx::ModelingLabel {
    ommx::ModelingLabel {
        name: Some(name.to_owned()),
        subscripts: vec![2, 5],
        parameters: [("axis".to_owned(), "row".to_owned())]
            .into_iter()
            .collect(),
        description: Some("bridge fixture".to_owned()),
    }
}

fn component_decision_variable() -> ommx::DecisionVariable {
    ommx::DecisionVariable::new(
        ommx::Kind::Integer,
        ommx::Bound::new(-2.0, 8.0).expect("the fixture bound is valid"),
        ommx::ATol::default(),
    )
    .expect("the fixture decision variable is valid")
}

fn component_instance() -> ommx::Instance {
    let id = ommx::VariableID::from(7);
    let mut labels = ommx::VariableLabelStore::new();
    labels.set_name(id, "instance_x");
    labels.set_subscripts(id, [9]);

    ommx::Instance::builder()
        .sense(ommx::Sense::Minimize)
        .objective(component_function())
        .decision_variables(BTreeMap::from([(id, component_decision_variable())]))
        .variable_labels(labels)
        .constraints(BTreeMap::new())
        .build()
        .expect("the fixture instance is valid")
}

fn component_state() -> ommx::v1::State {
    HashMap::from([(7, 4.0)]).into()
}

#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
fn function(py: Python<'_>) -> PyResult<PyFunction> {
    v2_target(py)?.transfer(py, component_function())
}

#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
fn composed_function(py: Python<'_>) -> PyResult<PyFunction> {
    v2_target(py)?.transfer(py, composed_component_function())
}

fn component_constraint() -> (ommx::Constraint, ommx::ConstraintContext) {
    (
        ommx::Constraint::less_than_or_equal_to_zero(component_function()),
        ommx::ConstraintContext {
            label: modeling_label("capacity"),
            provenance: vec![ommx::Provenance::OneHotConstraint(
                ommx::OneHotConstraintID::from(23),
            )],
        },
    )
}

#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
fn constraint(py: Python<'_>) -> PyResult<PyConstraint> {
    v2_target(py)?.transfer(py, component_constraint())
}

#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
fn composed_constraint(py: Python<'_>) -> PyResult<PyConstraint> {
    v2_target(py)?.transfer(
        py,
        (
            ommx::Constraint::less_than_or_equal_to_zero(composed_component_function()),
            ommx::ConstraintContext::default(),
        ),
    )
}

#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
fn decision_variable(py: Python<'_>) -> PyResult<PyDecisionVariable> {
    let id = ommx::VariableID::from(7);
    v2_target(py)?.transfer(py, (id, component_decision_variable(), modeling_label("x")))
}

#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
fn instance(py: Python<'_>) -> PyResult<PyInstance> {
    v2_target(py)?.transfer(py, component_instance())
}

#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
fn parametric_instance(py: Python<'_>) -> PyResult<PyParametricInstance> {
    v2_target(py)?.transfer(py, ParametricInstance::from(component_instance()))
}

#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
fn solution(py: Python<'_>) -> PyResult<PySolution> {
    let value = component_instance()
        .evaluate(&component_state(), ommx::ATol::default())
        .expect("the fixture state is valid");
    v2_target(py)?.transfer(py, value)
}

#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
fn sample_set(py: Python<'_>) -> PyResult<PySampleSet> {
    let value = component_instance()
        .evaluate_samples(
            &ommx::Sampled::from(component_state()),
            ommx::ATol::default(),
        )
        .expect("the fixture samples are valid");
    v2_target(py)?.transfer(py, value)
}

#[pymodule(gil_used = false)]
fn ommx_pyo3_bridge_fixture(module: &Bound<'_, PyModule>) -> PyResult<()> {
    transfer::register(module)?;
    module.add_function(wrap_pyfunction!(bridge_error_type, module)?)?;
    module.add_function(wrap_pyfunction!(function, module)?)?;
    module.add_function(wrap_pyfunction!(composed_function, module)?)?;
    module.add_function(wrap_pyfunction!(constraint, module)?)?;
    module.add_function(wrap_pyfunction!(composed_constraint, module)?)?;
    module.add_function(wrap_pyfunction!(decision_variable, module)?)?;
    module.add_function(wrap_pyfunction!(instance, module)?)?;
    module.add_function(wrap_pyfunction!(parametric_instance, module)?)?;
    module.add_function(wrap_pyfunction!(solution, module)?)?;
    module.add_function(wrap_pyfunction!(sample_set, module)?)?;
    Ok(())
}

pyo3_stub_gen::define_stub_info_gatherer!(stub_info);
