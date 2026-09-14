//! A consumer that compiles different representations after negotiating.

use super::*;
use ommx::Message;
use ommx_pyo3_bridge::{resolve_target, ProtobufV1, ProtobufV2};
use pyo3::{exceptions::PyImportError, types::PyBytes};

fn no_supported_protocol() -> PyErr {
    PyImportError::new_err("No supported OMMX transfer protocol")
}

#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction(signature = (after_resolve=None))]
fn negotiated_instance(
    py: Python<'_>,
    #[gen_stub(override_type(type_repr="collections.abc.Callable[[], None] | None", imports=("collections.abc",)))]
    after_resolve: Option<Py<PyAny>>,
) -> PyResult<PyInstance> {
    if let Some(target) = resolve_target::<ProtobufV2>(py)? {
        if let Some(callback) = after_resolve {
            callback.call0(py)?;
        }
        return target.transfer(py, component_instance());
    }
    if let Some(target) = resolve_target::<ProtobufV1>(py)? {
        if let Some(callback) = after_resolve {
            callback.call0(py)?;
        }
        let value: ommx::v1::Instance = component_instance().try_into().unwrap();
        return target.transfer(py, value);
    }
    Err(no_supported_protocol())
}

#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
fn completed_instance(
    py: Python<'_>,
    #[gen_stub(override_type(type_repr="collections.abc.Callable[[], None]", imports=("collections.abc",)))]
    after_transfer: Py<PyAny>,
) -> PyResult<(PyInstance, PyInstance)> {
    let value: PyInstance = resolve_target::<ProtobufV2>(py)?
        .ok_or_else(no_supported_protocol)?
        .transfer(py, component_instance())?;
    after_transfer.call0(py)?;
    // Completed output wrappers can be cloned without the GIL or a new transfer.
    let cloned = py.detach(|| value.clone());
    Ok((value, cloned))
}

fn variable_parts() -> (
    ommx::VariableID,
    ommx::DecisionVariable,
    ommx::ModelingLabel,
) {
    (7.into(), component_decision_variable(), modeling_label("x"))
}

fn hinted_instance() -> ommx::v1::Instance {
    let mut instance: ommx::v1::Instance = component_instance().try_into().unwrap();
    instance.decision_variables[0].kind = ommx::v1::decision_variable::Kind::Binary as i32;
    instance.decision_variables[0].bound = Some(ommx::Bound::of_binary().into());
    let mut constraint = ommx::v1::Constraint::default();
    constraint.id = 23;
    constraint.equality = ommx::v1::Equality::EqualToZero as i32;
    constraint.function =
        Some(ommx::Function::from((ommx::linear!(7) + ommx::coeff!(-1.0)).unwrap()).into());
    instance.constraints.push(constraint);
    let mut one_hot = ommx::v1::OneHot::default();
    one_hot.constraint_id = 23;
    one_hot.decision_variables = vec![7];
    let mut hints = ommx::v1::ConstraintHints::default();
    hints.one_hot_constraints.push(one_hot);
    instance.constraint_hints = Some(hints);
    instance
}

fn special_instance() -> ommx::Instance {
    let id = ommx::VariableID::from(7);
    ommx::Instance::builder()
        .sense(ommx::Sense::Minimize)
        .objective(component_function())
        .decision_variables(BTreeMap::from([(id, ommx::DecisionVariable::binary())]))
        .constraints(BTreeMap::new())
        .one_hot_constraints(BTreeMap::from([(
            ommx::OneHotConstraintID::from(23),
            ommx::OneHotConstraint::new([id].into_iter().collect()).unwrap(),
        )]))
        .build()
        .unwrap()
}

#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
fn compile_for_target(py: Python<'_>) -> PyResult<PyInstance> {
    if let Some(target) = resolve_target::<ProtobufV2>(py)? {
        return target.transfer(py, special_instance());
    }
    if let Some(target) = resolve_target::<ProtobufV1>(py)? {
        return target.transfer(py, hinted_instance());
    }
    Err(no_supported_protocol())
}

#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction(signature = (special=false))]
fn v1_first_instance(py: Python<'_>, special: bool) -> PyResult<PyInstance> {
    if let Some(target) = resolve_target::<ProtobufV1>(py)? {
        return target.transfer(
            py,
            if special {
                special_instance()
            } else {
                component_instance()
            },
        );
    }
    if let Some(target) = resolve_target::<ProtobufV2>(py)? {
        return target.transfer(py, component_instance());
    }
    Err(no_supported_protocol())
}

// Decode at the Rust wire boundary to check preservation of advisory v1 data.
#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
fn has_legacy_hint(bytes: &Bound<'_, PyBytes>) -> bool {
    let actual = ommx::v1::Instance::decode(bytes.as_bytes()).unwrap();
    actual == hinted_instance()
}

#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
fn invalid_instance(py: Python<'_>) -> PyResult<PyInstance> {
    let target = resolve_target::<ProtobufV1>(py)?.ok_or_else(no_supported_protocol)?;
    target.transfer(py, ommx::v1::Instance::default())
}

#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
fn v1_function(py: Python<'_>) -> PyResult<PyFunction> {
    let target = resolve_target::<ProtobufV1>(py)?.ok_or_else(no_supported_protocol)?;
    target.transfer(py, component_function())
}

#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
fn v1_constraint(py: Python<'_>) -> PyResult<PyConstraint> {
    let target = resolve_target::<ProtobufV1>(py)?.ok_or_else(no_supported_protocol)?;
    let mut value = hinted_instance().constraints.remove(0);
    value.name = Some("choice".to_owned());
    target.transfer(py, value)
}

#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction(signature = (fixed=false))]
fn v1_decision_variable(py: Python<'_>, fixed: bool) -> PyResult<PyDecisionVariable> {
    let target = resolve_target::<ProtobufV1>(py)?.ok_or_else(no_supported_protocol)?;
    if fixed {
        let mut variable = hinted_instance().decision_variables.remove(0);
        variable.substituted_value = Some(1.0);
        target.transfer(py, variable)
    } else {
        target.transfer(py, variable_parts())
    }
}

macro_rules! negotiated_root {
    ($name:ident, $wrapper:ident, $wire:ident, $source:expr) => {
        #[pyo3_stub_gen::derive::gen_stub_pyfunction]
        #[pyfunction]
        fn $name(py: Python<'_>) -> PyResult<$wrapper> {
            if let Some(target) = resolve_target::<ProtobufV2>(py)? {
                return target.transfer(py, $source);
            }
            if let Some(target) = resolve_target::<ProtobufV1>(py)? {
                // Build a v1 root explicitly. This fixture has no special results.
                let value: ommx::v1::$wire = ($source).into();
                return target.transfer(py, value);
            }
            Err(no_supported_protocol())
        }
    };
}
// ParametricInstance has a checked v1 conversion, unlike the result roots.
#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
fn negotiated_parametric_instance(py: Python<'_>) -> PyResult<PyParametricInstance> {
    if let Some(target) = resolve_target::<ProtobufV2>(py)? {
        return target.transfer(py, ParametricInstance::from(component_instance()));
    }
    if let Some(target) = resolve_target::<ProtobufV1>(py)? {
        return target.transfer(py, ParametricInstance::from(component_instance()));
    }
    Err(no_supported_protocol())
}
negotiated_root!(
    negotiated_solution,
    PySolution,
    Solution,
    component_instance()
        .evaluate(&component_state(), ommx::ATol::default())
        .unwrap()
);
negotiated_root!(
    negotiated_sample_set,
    PySampleSet,
    SampleSet,
    component_instance()
        .evaluate_samples(
            &ommx::Sampled::from(component_state()),
            ommx::ATol::default()
        )
        .unwrap()
);

#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
fn v2_components(py: Python<'_>) -> PyResult<(PyFunction, PyConstraint, PyDecisionVariable)> {
    let target = resolve_target::<ProtobufV2>(py)?.ok_or_else(no_supported_protocol)?;
    Ok((
        target.transfer(py, component_function())?,
        target.transfer(py, component_constraint())?,
        target.transfer(py, variable_parts())?,
    ))
}

pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(negotiated_instance, module)?)?;
    module.add_function(wrap_pyfunction!(completed_instance, module)?)?;
    module.add_function(wrap_pyfunction!(compile_for_target, module)?)?;
    module.add_function(wrap_pyfunction!(v1_first_instance, module)?)?;
    module.add_function(wrap_pyfunction!(has_legacy_hint, module)?)?;
    module.add_function(wrap_pyfunction!(invalid_instance, module)?)?;
    module.add_function(wrap_pyfunction!(v1_function, module)?)?;
    module.add_function(wrap_pyfunction!(v1_constraint, module)?)?;
    module.add_function(wrap_pyfunction!(v1_decision_variable, module)?)?;
    module.add_function(wrap_pyfunction!(negotiated_parametric_instance, module)?)?;
    module.add_function(wrap_pyfunction!(negotiated_solution, module)?)?;
    module.add_function(wrap_pyfunction!(negotiated_sample_set, module)?)?;
    module.add_function(wrap_pyfunction!(v2_components, module)?)?;
    Ok(())
}
