use crate::{
    coeff, mps::*, quadratic, Constraint, ConstraintHints, ConstraintID, DecisionVariable,
    Function, Instance, Sense,
};
use maplit::btreemap;
use std::collections::BTreeMap;

// Test error cases for MPS write operations
#[test]
fn test_nonlinear_objective_error() {
    let decision_variables = btreemap! {
        VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
        VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
    };
    // x1 * x2 + 1
    let objective = (quadratic!(1, 2) + coeff!(1.0)).into();
    let instance = Instance::new(
        Sense::Minimize,
        objective,
        decision_variables,
        BTreeMap::new(),
        ConstraintHints::default(),
    )
    .unwrap();

    let mut buffer = Vec::new();
    let result = format::format(&instance, &mut buffer);
    assert!(matches!(
        result.unwrap_err(),
        MpsWriteError::InvalidObjectiveType { degree: 2 }
    ));
}

#[test]
fn test_nonlinear_constraint_error() {
    let decision_variables = btreemap! {
        VariableID::from(0) => DecisionVariable::continuous(VariableID::from(0))
    };

    // Create constraint with quadratic function: x^2 <= 0
    let quadratic_function = quadratic!(0, 0).into();
    let constraints = btreemap! {
        ConstraintID::from(0) => Constraint::less_than_or_equal_to_zero(
            ConstraintID::from(0),
            quadratic_function
        ),
    };

    let instance = Instance::new(
        Sense::Minimize,
        Function::Zero, // Linear objective
        decision_variables,
        constraints,
        ConstraintHints::default(),
    )
    .unwrap();

    let mut buffer = Vec::new();
    let result = format::format(&instance, &mut buffer);
    assert!(matches!(
        result.unwrap_err(),
        MpsWriteError::InvalidConstraintType { name, degree: 2 } if name == "OMMX_CONSTR_0"
    ));
}
