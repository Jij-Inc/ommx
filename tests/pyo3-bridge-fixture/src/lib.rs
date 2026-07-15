use ommx_pyo3_bridge::{PyConstraint, PyDecisionVariable, PyFunction, PyInstance};
use pyo3::prelude::*;
use std::collections::BTreeMap;

fn component_function() -> ommx::Function {
    let linear = (ommx::linear!(7) + ommx::coeff!(-3.0))
        .expect("the fixture uses finite, non-zero coefficients");
    ommx::Function::from(linear)
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

#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
fn function() -> PyFunction {
    component_function().into()
}

#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
fn constraint() -> PyConstraint {
    PyConstraint::new(
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
fn decision_variable() -> PyDecisionVariable {
    let id = ommx::VariableID::from(7);
    PyDecisionVariable::new(id, component_decision_variable(), modeling_label("x"))
}

#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
fn instance() -> PyInstance {
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
        .into()
}

#[pymodule(gil_used = false)]
fn ommx_pyo3_bridge_fixture(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(function, module)?)?;
    module.add_function(wrap_pyfunction!(constraint, module)?)?;
    module.add_function(wrap_pyfunction!(decision_variable, module)?)?;
    module.add_function(wrap_pyfunction!(instance, module)?)?;
    Ok(())
}

pyo3_stub_gen::define_stub_info_gatherer!(stub_info);
