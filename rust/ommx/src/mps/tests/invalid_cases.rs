use super::super::*;

// Test error cases for MPS write operations
#[test]
fn test_nonlinear_objective_error() {
    // Create instance with linear part
    let mut instance = crate::v1::Instance::default();
    instance
        .decision_variables
        .push(crate::v1::DecisionVariable {
            id: 0,
            name: Some("x".to_string()),
            kind: crate::v1::decision_variable::Kind::Continuous as i32,
            bound: Some(crate::v1::Bound {
                lower: 0.0,
                upper: f64::INFINITY,
            }),
            ..Default::default()
        });

    // Create a function with degree > 1 (quadratic term)
    let mut func = crate::v1::Function::default();
    func.function = Some(crate::v1::function::Function::Quadratic(
        crate::v1::Quadratic {
            rows: vec![0],
            columns: vec![0],
            values: vec![1.0],
            linear: None,
        },
    ));

    instance.objective = Some(func);

    let mut buffer = Vec::new();
    let result = format::format(&instance, &mut buffer);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        MpsWriteError::InvalidObjectiveType { degree: 2 }
    ));
}

#[test]
fn test_nonlinear_constraint_error() {
    // Create instance with linear variable
    let mut instance = crate::v1::Instance::default();
    instance
        .decision_variables
        .push(crate::v1::DecisionVariable {
            id: 0,
            name: Some("x".to_string()),
            kind: crate::v1::decision_variable::Kind::Continuous as i32,
            bound: Some(crate::v1::Bound {
                lower: 0.0,
                upper: f64::INFINITY,
            }),
            ..Default::default()
        });

    // Create constraint with quadratic function
    let mut func = crate::v1::Function::default();
    func.function = Some(crate::v1::function::Function::Quadratic(
        crate::v1::Quadratic {
            rows: vec![0],
            columns: vec![0],
            values: vec![1.0],
            linear: None,
        },
    ));

    instance.constraints.push(crate::v1::Constraint {
        id: 0,
        name: Some("quad_constraint".to_string()),
        equality: crate::v1::Equality::LessThanOrEqualToZero as i32,
        function: Some(func),
        ..Default::default()
    });

    let mut buffer = Vec::new();
    let result = format::format(&instance, &mut buffer);
    assert!(result.is_err());
    assert!(
        matches!(result.unwrap_err(), MpsWriteError::InvalidConstraintType { name, degree: 2 } if name == "OMMX_CONSTR_0")
    );
}

#[test]
fn test_cubic_polynomial_error() {
    // Test with degree 3 polynomial
    let mut instance = crate::v1::Instance::default();
    instance
        .decision_variables
        .push(crate::v1::DecisionVariable {
            id: 0,
            name: Some("x".to_string()),
            kind: crate::v1::decision_variable::Kind::Continuous as i32,
            bound: Some(crate::v1::Bound {
                lower: 0.0,
                upper: f64::INFINITY,
            }),
            ..Default::default()
        });

    // Create a polynomial function (this would be degree 3 or higher)
    let mut func = crate::v1::Function::default();
    func.function = Some(crate::v1::function::Function::Polynomial(
        crate::v1::Polynomial {
            terms: vec![crate::v1::Monomial {
                coefficient: 1.0,
                ids: vec![0, 0, 0], // x^3 term
            }],
        },
    ));

    instance.objective = Some(func);

    let mut buffer = Vec::new();
    let result = format::format(&instance, &mut buffer);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        MpsWriteError::InvalidObjectiveType { degree: 3 }
    ));
}

#[test]
fn test_invalid_variable_id_error() {
    // Create instance with missing variable reference
    let mut instance = crate::v1::Instance::default();

    // Create objective that references non-existent variable id 99
    let mut func = crate::v1::Function::default();
    func.function = Some(crate::v1::function::Function::Linear(crate::v1::Linear {
        terms: vec![crate::v1::linear::Term {
            id: 99, // This variable doesn't exist
            coefficient: 1.0,
        }],
        constant: 0.0,
    }));

    instance.objective = Some(func);

    let mut buffer = Vec::new();
    let result = format::format(&instance, &mut buffer);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(
        matches!(error, MpsWriteError::InvalidVariableId(var_id) if var_id == crate::VariableID::from(99))
    );
}
