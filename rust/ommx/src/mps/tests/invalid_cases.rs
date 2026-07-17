use crate::{
    mps::*, quadratic, Constraint, ConstraintID, DecisionVariable, Function, Instance, Sense,
    VariableID,
};
use maplit::btreemap;
use std::collections::BTreeMap;

// Test error cases for MPS write operations with higher-degree polynomials
#[test]
fn test_nonlinear_objective_error() {
    let decision_variables = btreemap! {
        VariableID::from(1) => DecisionVariable::binary(),
        VariableID::from(2) => DecisionVariable::binary(),
    };
    // Create a cubic function: x1 * x2 * x1 (degree 3, not supported)
    let cubic_function = (quadratic!(1, 2) * quadratic!(1)).into();
    let instance = Instance::new(
        Sense::Minimize,
        cubic_function,
        decision_variables,
        BTreeMap::new(),
    )
    .unwrap();

    let mut buffer = Vec::new();
    let err = format::format(&instance, &mut buffer).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("nonlinear objective") && msg.contains("3-degree"),
        "unexpected error: {msg}"
    );
}

#[test]
fn test_nonlinear_constraint_error() {
    let decision_variables = btreemap! {
        VariableID::from(0) => DecisionVariable::continuous()
    };

    // Create constraint with cubic function: x^3 <= 0 (degree 3, not supported)
    let cubic_function = (quadratic!(0, 0) * quadratic!(0)).into();
    let constraints = btreemap! {
        ConstraintID::from(0) => Constraint::less_than_or_equal_to_zero(cubic_function
        ),
    };

    let instance = Instance::new(
        Sense::Minimize,
        Function::Zero, // Linear objective
        decision_variables,
        constraints,
    )
    .unwrap();

    let mut buffer = Vec::new();
    let err = format::format(&instance, &mut buffer).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("nonlinear constraint")
            && msg.contains("OMMX_CONSTR_0")
            && msg.contains("3-degree"),
        "unexpected error: {msg}"
    );
}

#[test]
fn test_finite_domain_variable_error() {
    let decision_variables = btreemap! {
        VariableID::from(0) => DecisionVariable::new_finite_domain(vec![0.1, 0.5, 1.0]).unwrap(),
    };
    let instance = Instance::new(
        Sense::Minimize,
        Function::from(crate::linear!(0)),
        decision_variables,
        BTreeMap::new(),
    )
    .unwrap();

    let mut buffer = Vec::new();
    let error = format::format(&instance, &mut buffer).unwrap_err();

    assert!(
        error
            .to_string()
            .contains("MPS format does not support finite-domain decision variable"),
        "unexpected error: {error}"
    );
}
