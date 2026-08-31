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

    let mut buffer = b"unchanged".to_vec();
    let err = format::format(&instance, &mut buffer).unwrap_err();
    insta::assert_snapshot!(err.to_string(), @r###"
    Instance is outside the MPS input class:
    Instance does not belong to any clause:
    - clause 0 (`MPS`):
      - objective degree 3 exceeds degree <= 2
    "###);
    assert_eq!(buffer, b"unchanged");
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

    let mut buffer = b"unchanged".to_vec();
    let err = format::format(&instance, &mut buffer).unwrap_err();
    insta::assert_snapshot!(err.to_string(), @r###"
    Instance is outside the MPS input class:
    Instance does not belong to any clause:
    - clause 0 (`MPS`):
      - regular LessThanOrEqualToZero constraint degrees {ConstraintID(0): Degree(3)} exceed degree <= 2
    "###);
    assert_eq!(buffer, b"unchanged");
}
