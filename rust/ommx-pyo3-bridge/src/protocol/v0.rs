//! Sender implementation for bridge protocol v0.
//!
//! This protocol identifies the endpoint names, call signatures, and protobuf
//! payload interpretation used to reconstruct canonical Python OMMX classes.
//! It intentionally performs no protocol negotiation.

use ommx::Message as _;
use pyo3::{
    exceptions::PyImportError,
    prelude::*,
    types::{PyAny, PyBytes},
};

const FUNCTION_ENDPOINT: &str = "_pyo3_bridge_v0_function_from_bytes";
const CONSTRAINT_ENDPOINT: &str = "_pyo3_bridge_v0_constraint_from_bytes";
const DECISION_VARIABLE_ENDPOINT: &str = "_pyo3_bridge_v0_decision_variable_from_bytes";

fn incompatible_python_ommx(capability: &str, source: PyErr) -> PyErr {
    PyImportError::new_err(format!(
        "the installed Python OMMX package does not provide the required bridge capability \
         `{capability}`; install a Python OMMX release compatible with this \
         `ommx-pyo3-bridge` crate ({source})"
    ))
}

fn incompatible_python_ommx_root(class_name: &str, source: PyErr) -> PyErr {
    incompatible_python_ommx(&format!("ommx.{class_name}.from_v2_bytes"), source)
}

fn bridge_endpoint<'py>(py: Python<'py>, endpoint: &str) -> PyResult<Bound<'py, PyAny>> {
    let module = py
        .import("ommx._ommx_rust")
        .map_err(|error| incompatible_python_ommx(endpoint, error))?;
    module
        .getattr(endpoint)
        .map_err(|error| incompatible_python_ommx(endpoint, error))
}

fn root_from_v2_bytes<'py>(py: Python<'py>, class_name: &str) -> PyResult<Bound<'py, PyAny>> {
    let module = py
        .import("ommx")
        .map_err(|error| incompatible_python_ommx_root(class_name, error))?;
    let root_class = module
        .getattr(class_name)
        .map_err(|error| incompatible_python_ommx_root(class_name, error))?;
    root_class
        .getattr("from_v2_bytes")
        .map_err(|error| incompatible_python_ommx_root(class_name, error))
}

fn function_payload(function: ommx::Function) -> Vec<u8> {
    function.to_bytes()
}

fn constraint_payloads(
    constraint: ommx::Constraint,
    context: ommx::ConstraintContext,
) -> (Vec<u8>, Vec<u8>) {
    (
        ommx::v2::RegularConstraint::from(constraint).encode_to_vec(),
        ommx::v2::ConstraintContext::from(context).encode_to_vec(),
    )
}

fn decision_variable_payloads(
    id: ommx::VariableID,
    decision_variable: ommx::DecisionVariable,
    label: ommx::ModelingLabel,
) -> (u64, Vec<u8>, Vec<u8>) {
    (
        id.into(),
        ommx::v2::DecisionVariable::from(decision_variable).encode_to_vec(),
        ommx::v2::ModelingLabel::from(label).encode_to_vec(),
    )
}

pub fn function_into_py<'py>(
    function: ommx::Function,
    py: Python<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let bytes = function_payload(function);
    bridge_endpoint(py, FUNCTION_ENDPOINT)?.call1((PyBytes::new(py, &bytes),))
}

pub fn constraint_into_py<'py>(
    constraint: ommx::Constraint,
    context: ommx::ConstraintContext,
    py: Python<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let (constraint, context) = constraint_payloads(constraint, context);
    bridge_endpoint(py, CONSTRAINT_ENDPOINT)?
        .call1((PyBytes::new(py, &constraint), PyBytes::new(py, &context)))
}

pub fn decision_variable_into_py<'py>(
    id: ommx::VariableID,
    decision_variable: ommx::DecisionVariable,
    label: ommx::ModelingLabel,
    py: Python<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let (id, decision_variable, label) = decision_variable_payloads(id, decision_variable, label);
    bridge_endpoint(py, DECISION_VARIABLE_ENDPOINT)?.call1((
        id,
        PyBytes::new(py, &decision_variable),
        PyBytes::new(py, &label),
    ))
}

pub fn root_into_py<'py>(
    bytes: Vec<u8>,
    class_name: &str,
    py: Python<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    root_from_v2_bytes(py, class_name)?.call1((PyBytes::new(py, &bytes),))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ommx::Parse as _;
    use proptest::{prelude::*, string::string_regex};

    fn arbitrary_function() -> impl Strategy<Value = ommx::Function> {
        prop_oneof![
            Just(ommx::Function::Zero),
            any::<ommx::Coefficient>().prop_map(ommx::Function::Constant),
            any::<ommx::Linear>().prop_map(ommx::Function::Linear),
            any::<ommx::Quadratic>().prop_map(ommx::Function::Quadratic),
            any::<ommx::Polynomial>().prop_map(ommx::Function::Polynomial),
        ]
    }

    fn arbitrary_constraint() -> impl Strategy<Value = ommx::Constraint> {
        (arbitrary_function(), any::<ommx::Equality>()).prop_map(|(function, equality)| {
            match equality {
                ommx::Equality::EqualToZero => ommx::Constraint::equal_to_zero(function),
                ommx::Equality::LessThanOrEqualToZero => {
                    ommx::Constraint::less_than_or_equal_to_zero(function)
                }
            }
        })
    }

    fn short_string() -> impl Strategy<Value = String> {
        string_regex("[a-z]{0,12}").expect("the test regex is valid")
    }

    fn arbitrary_label() -> impl Strategy<Value = ommx::ModelingLabel> {
        (
            proptest::option::of(short_string()),
            proptest::collection::vec(any::<i64>(), 0..5),
            proptest::collection::vec((short_string(), short_string()), 0..5),
            proptest::option::of(short_string()),
        )
            .prop_map(
                |(name, subscripts, parameters, description)| ommx::ModelingLabel {
                    name,
                    subscripts,
                    parameters: parameters.into_iter().collect(),
                    description,
                },
            )
    }

    fn arbitrary_provenance() -> impl Strategy<Value = ommx::Provenance> {
        prop_oneof![
            any::<u64>().prop_map(|id| ommx::Provenance::IndicatorConstraint(id.into())),
            any::<u64>().prop_map(|id| ommx::Provenance::OneHotConstraint(id.into())),
            any::<u64>().prop_map(|id| ommx::Provenance::Sos1Constraint(id.into())),
        ]
    }

    fn arbitrary_context() -> impl Strategy<Value = ommx::ConstraintContext> {
        (
            arbitrary_label(),
            proptest::collection::vec(arbitrary_provenance(), 0..5),
        )
            .prop_map(|(label, provenance)| ommx::ConstraintContext { label, provenance })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn function_payload_preserves_all_function_variants(function in arbitrary_function()) {
            let expected = function.clone();
            let payload = function_payload(function);
            let actual = ommx::Function::from_bytes(&payload).unwrap();
            prop_assert_eq!(actual, expected);
        }

        #[test]
        fn constraint_payloads_preserve_intrinsic_and_owner_data(
            constraint in arbitrary_constraint(),
            context in arbitrary_context(),
        ) {
            let expected_constraint = constraint.clone();
            let expected_context = context.clone();
            let (constraint, context) = constraint_payloads(constraint, context);
            let actual_constraint = ommx::v2::RegularConstraint::decode(constraint.as_slice())
                .unwrap()
                .parse(&())
                .unwrap();
            let actual_context = ommx::v2::ConstraintContext::decode(context.as_slice())
                .unwrap()
                .parse(&())
                .unwrap();
            prop_assert_eq!(actual_constraint, expected_constraint);
            prop_assert_eq!(actual_context, expected_context);
        }

        #[test]
        fn decision_variable_payloads_preserve_identity_intrinsic_and_owner_data(
            id in any::<u64>(),
            decision_variable in any::<ommx::DecisionVariable>(),
            label in arbitrary_label(),
        ) {
            let expected_decision_variable = decision_variable.clone();
            let expected_label = label.clone();
            let (actual_id, decision_variable, label) = decision_variable_payloads(
                id.into(),
                decision_variable,
                label,
            );
            let actual_decision_variable = ommx::v2::DecisionVariable::decode(
                decision_variable.as_slice(),
            )
            .unwrap()
            .parse(&ommx::VariableID::from(id))
            .unwrap();
            let actual_label: ommx::ModelingLabel =
                ommx::v2::ModelingLabel::decode(label.as_slice()).unwrap().into();
            prop_assert_eq!(actual_id, id);
            prop_assert_eq!(actual_decision_variable, expected_decision_variable);
            prop_assert_eq!(actual_label, expected_label);
        }

        #[test]
        fn instance_payload_preserves_owner_complete_root(instance in any::<ommx::Instance>()) {
            let expected = instance.clone();
            let payload = instance.to_v2_bytes();
            let actual = ommx::Instance::from_v2_bytes(&payload).unwrap();
            prop_assert_eq!(actual, expected);
        }
    }
}
