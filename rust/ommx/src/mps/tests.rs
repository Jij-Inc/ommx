use super::*;
use crate::{random::InstanceParameters, v1::Instance};
use approx::AbsDiffEq;
use proptest::prelude::*;
use std::io::Write;
use tempdir::TempDir;

proptest! {
    #[test]
    fn test_write_mps(instance in Instance::arbitrary_with(InstanceParameters::default_lp())) {
        let mut buffer = Vec::new();
        prop_assert!(to_mps::write_mps(&instance, &mut buffer).is_ok())
    }

    #[test]
    fn test_roundtrip(instance in Instance::arbitrary_with(InstanceParameters::default_lp())) {
        let mut buffer = Vec::new();
        prop_assert!(to_mps::write_mps(&instance, &mut buffer).is_ok());
        let loaded_instance = load_raw_reader(&buffer[..]).unwrap();
        dbg!(&instance);
        prop_assert!(instance.abs_diff_eq(&dbg!(loaded_instance), crate::ATol::default()));
    }
}

const MPS_CONTENT: &str = r#"NAME TestProblem
ROWS
 N  OBJ
 L  R1
COLUMNS
    X1        OBJ                 1
    X1        R1                  1
RHS
    RHS1      R1                  5
BOUNDS
 UP BND1      X1                  4
ENDATA
"#;

// More complex MPS test case with multiple variables and constraints
const MPS_COMPLEX: &str = r#"NAME ComplexProblem
ROWS
 N  OBJ
 L  C1
 G  C2
 E  C3
COLUMNS
    X1        OBJ                 1   C1                  2
    X1        C2                  1   C3                  1
    X2        OBJ                 4   C1                  1
    X2        C3                 -1
    X3        OBJ                 9   C2                  1
    X3        C3                  1
RHS
    RHS1      C1                  5   C2                 10
    RHS1      C3                  7
BOUNDS
 UP BND1      X1                  4
 LO BND1      X2                 -1
 UP BND1      X2                  1
ENDATA
"#;

// MPS with RANGES section
const MPS_WITH_RANGES: &str = r#"NAME RangesProblem
ROWS
 N  OBJ
 L  R1
 G  R2
COLUMNS
    X1        OBJ                 1   R1                  1
    X1        R2                  1
    X2        OBJ                 2   R1                  2
    X2        R2                  1
RHS
    RHS1      R1                 10   R2                  5
RANGES
    RNG1      R1                  2   R2                  3
ENDATA
"#;

// MPS with integer variables
const MPS_INTEGER: &str = r#"NAME IntegerProblem
ROWS
 N  OBJ
 L  C1
COLUMNS
    MARK0000  'MARKER'                 'INTORG'
    X1        OBJ                 1   C1                  1
    X2        OBJ                 2   C1                  1
    MARK0001  'MARKER'                 'INTEND'
    X3        OBJ                 3   C1                  1
RHS
    RHS1      C1                 10
BOUNDS
 UI BND1      X1                  5
 UI BND1      X2                  5
 UP BND1      X3                  5
ENDATA
"#;

// MPS with binary variables
const MPS_BINARY: &str = r#"NAME BinaryProblem
ROWS
 N  OBJ
 L  C1
COLUMNS
    X1        OBJ                 1   C1                  1
    X2        OBJ                 2   C1                  1
RHS
    RHS1      C1                  1
BOUNDS
 BV BND1      X1
 BV BND1      X2
ENDATA
"#;

// MPS with free variables
const MPS_FREE_VAR: &str = r#"NAME FreeVarProblem
ROWS
 N  OBJ
 E  C1
COLUMNS
    X1        OBJ                 1   C1                  1
    X2        OBJ                -1   C1                 -1
RHS
    RHS1      C1                  0
BOUNDS
 FR BND1      X1
 FR BND1      X2
ENDATA
"#;

// MPS with OBJSENSE
const MPS_MAXIMIZE: &str = r#"NAME MaximizeProblem
OBJSENSE
 MAX
ROWS
 N  OBJ
 L  C1
COLUMNS
    X1        OBJ                 1   C1                  1
    X2        OBJ                 2   C1                  1
RHS
    RHS1      C1                 10
ENDATA
"#;

#[test]
fn test_format_detection() {
    let temp_dir = TempDir::new("test_mps_format_detection").unwrap();
    let temp_dir_path = temp_dir.path();
    let uncompressed_path = temp_dir_path.join("test.mps");
    let compressed_path = temp_dir_path.join("test.mps.gz");

    // Create uncompressed file
    std::fs::write(&uncompressed_path, MPS_CONTENT).unwrap();

    // Create compressed file
    {
        let file = std::fs::File::create(&compressed_path).unwrap();
        let mut encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        encoder.write_all(MPS_CONTENT.as_bytes()).unwrap();
        encoder.finish().unwrap();
    }

    let uncompressed = load_file(&uncompressed_path).unwrap();
    let compressed = load_file(&compressed_path).unwrap();
    assert_eq!(compressed, uncompressed);
}

// Test basic MPS parsing
#[test]
fn test_basic_mps_parsing() {
    let instance = load_raw_reader(MPS_CONTENT.as_bytes()).unwrap();
    
    // Check instance properties
    assert_eq!(instance.sense(), crate::v1::instance::Sense::Minimize);
    assert_eq!(instance.decision_variables.len(), 1);
    assert_eq!(instance.constraints.len(), 1);
    
    // Check variable
    let var = &instance.decision_variables[0];
    assert_eq!(var.name(), "X1");
    assert_eq!(var.kind(), crate::v1::decision_variable::Kind::Continuous);
    assert!(var.bound.is_some());
    
    let bound = var.bound.as_ref().unwrap();
    assert_eq!(bound.lower, 0.0);
    assert_eq!(bound.upper, 4.0);
    
    // Check constraint
    let constraint = &instance.constraints[0];
    assert_eq!(constraint.name(), "R1");
    let linear = constraint.function().into_owned().as_linear().unwrap();
    assert_eq!(linear.terms.len(), 1);
    assert_eq!(constraint.equality(), crate::v1::Equality::LessThanOrEqualToZero);
    assert_eq!(linear.constant, -5.0); // RHS is stored as negative constant
}

// Test complex MPS with all constraint types
#[test]
fn test_complex_mps_parsing() {
    let instance = load_raw_reader(MPS_COMPLEX.as_bytes()).unwrap();
    
    assert_eq!(instance.decision_variables.len(), 3);
    assert_eq!(instance.constraints.len(), 3);
    
    // Check variables
    assert_eq!(instance.decision_variables[0].name(), "X1");
    assert_eq!(instance.decision_variables[1].name(), "X2");
    assert_eq!(instance.decision_variables[2].name(), "X3");
    
    // X1: 0 <= x <= 4
    let x1_bound = instance.decision_variables[0].bound.as_ref().unwrap();
    assert_eq!(x1_bound.lower, 0.0);
    assert_eq!(x1_bound.upper, 4.0);
    
    // X2: -1 <= x <= 1
    let x2_bound = instance.decision_variables[1].bound.as_ref().unwrap();
    assert_eq!(x2_bound.lower, -1.0);
    assert_eq!(x2_bound.upper, 1.0);
    
    // X3: no explicit bounds (0 <= x < inf)
    let x3_bound = instance.decision_variables[2].bound.as_ref().unwrap();
    assert_eq!(x3_bound.lower, 0.0);
    assert_eq!(x3_bound.upper, f64::INFINITY);
    
    // Check constraints
    let c1 = &instance.constraints[0];
    assert_eq!(c1.name(), "C1");
    assert_eq!(c1.equality(), crate::v1::Equality::LessThanOrEqualToZero);
    let c1_linear = c1.function().into_owned().as_linear().unwrap();
    assert_eq!(c1_linear.constant, -5.0); // RHS stored as negative constant
    
    let c2 = &instance.constraints[1];
    assert_eq!(c2.name(), "C2");
    assert_eq!(c2.equality(), crate::v1::Equality::LessThanOrEqualToZero);
    let c2_linear = c2.function().into_owned().as_linear().unwrap();
    assert_eq!(c2_linear.constant, 10.0); // GE becomes LE with negated coefficients
    
    let c3 = &instance.constraints[2];
    assert_eq!(c3.name(), "C3");
    assert_eq!(c3.equality(), crate::v1::Equality::EqualToZero);
    let c3_linear = c3.function().into_owned().as_linear().unwrap();
    assert_eq!(c3_linear.constant, -7.0); // RHS stored as negative constant
}

// Test MPS with RANGES section
#[test]
fn test_mps_with_ranges() {
    let instance = load_raw_reader(MPS_WITH_RANGES.as_bytes()).unwrap();
    
    assert_eq!(instance.constraints.len(), 2);
    
    // R1 is L type with RHS=10 and range=2
    // Range processing creates additional constraints
    let r1 = &instance.constraints[0];
    assert_eq!(r1.name(), "R1");
    assert_eq!(r1.equality(), crate::v1::Equality::LessThanOrEqualToZero);
    
    // R2 is G type with RHS=5 and range=3
    // Range processing creates additional constraints
    let r2 = &instance.constraints[1];
    assert_eq!(r2.name(), "R2");
    assert_eq!(r2.equality(), crate::v1::Equality::LessThanOrEqualToZero);
}

// Test integer variables
#[test]
fn test_integer_variables() {
    let instance = load_raw_reader(MPS_INTEGER.as_bytes()).unwrap();
    
    assert_eq!(instance.decision_variables.len(), 3);
    
    // X1 and X2 should be integer
    assert_eq!(instance.decision_variables[0].kind(), crate::v1::decision_variable::Kind::Integer);
    assert_eq!(instance.decision_variables[1].kind(), crate::v1::decision_variable::Kind::Integer);
    
    // X3 should be continuous
    assert_eq!(instance.decision_variables[2].kind(), crate::v1::decision_variable::Kind::Continuous);
    
    // Check UI bounds
    let x1_bound = instance.decision_variables[0].bound.as_ref().unwrap();
    assert_eq!(x1_bound.upper, 5.0);
    
    let x2_bound = instance.decision_variables[1].bound.as_ref().unwrap();
    assert_eq!(x2_bound.upper, 5.0);
}

// Test binary variables
#[test]
fn test_binary_variables() {
    let instance = load_raw_reader(MPS_BINARY.as_bytes()).unwrap();
    
    assert_eq!(instance.decision_variables.len(), 2);
    
    // Both should be binary
    assert_eq!(instance.decision_variables[0].kind(), crate::v1::decision_variable::Kind::Binary);
    assert_eq!(instance.decision_variables[1].kind(), crate::v1::decision_variable::Kind::Binary);
    
    // Binary variables should have bounds [0, 1]
    let x1_bound = instance.decision_variables[0].bound.as_ref().unwrap();
    assert_eq!(x1_bound.lower, 0.0);
    assert_eq!(x1_bound.upper, 1.0);
}

// Test free variables
#[test]
fn test_free_variables() {
    let instance = load_raw_reader(MPS_FREE_VAR.as_bytes()).unwrap();
    
    assert_eq!(instance.decision_variables.len(), 2);
    
    // Both should be continuous with -inf to +inf bounds
    let x1_bound = instance.decision_variables[0].bound.as_ref().unwrap();
    assert_eq!(x1_bound.lower, f64::NEG_INFINITY);
    assert_eq!(x1_bound.upper, f64::INFINITY);
    
    let x2_bound = instance.decision_variables[1].bound.as_ref().unwrap();
    assert_eq!(x2_bound.lower, f64::NEG_INFINITY);
    assert_eq!(x2_bound.upper, f64::INFINITY);
}

// Test OBJSENSE
#[test]
fn test_objsense_maximize() {
    let instance = load_raw_reader(MPS_MAXIMIZE.as_bytes()).unwrap();
    
    assert_eq!(instance.sense(), crate::v1::instance::Sense::Maximize);
}

// Test roundtrip for all test cases
#[test]
fn test_roundtrip_all_cases() {
    let test_cases = vec![
        ("basic", MPS_CONTENT),
        ("complex", MPS_COMPLEX),
        ("integer", MPS_INTEGER),
        ("binary", MPS_BINARY),
        ("free_var", MPS_FREE_VAR),
        ("maximize", MPS_MAXIMIZE),
    ];
    
    for (name, mps_str) in test_cases {
        let original = load_raw_reader(mps_str.as_bytes()).unwrap();
        
        let mut buffer = Vec::new();
        to_mps::write_mps(&original, &mut buffer).unwrap();
        
        let roundtrip = load_raw_reader(&buffer[..]).unwrap();
        
        assert!(
            original.abs_diff_eq(&roundtrip, crate::ATol::default()),
            "Roundtrip failed for test case: {}",
            name
        );
    }
}

// Test error cases for MPS write operations
#[test]
fn test_nonlinear_objective_error() {
    // Create instance with linear part
    let mut instance = crate::v1::Instance::default();
    instance.decision_variables.push(crate::v1::DecisionVariable {
        id: 0,
        name: Some("x".to_string()),
        kind: crate::v1::decision_variable::Kind::Continuous as i32,
        bound: Some(crate::v1::Bound { lower: 0.0, upper: f64::INFINITY }),
        ..Default::default()
    });
    
    // Create a function with degree > 1 (quadratic term)
    let mut func = crate::v1::Function::default();
    func.function = Some(crate::v1::function::Function::Quadratic(crate::v1::Quadratic {
        rows: vec![0],
        columns: vec![0], 
        values: vec![1.0],
        linear: None,
    }));
    
    instance.objective = Some(func);
    
    let mut buffer = Vec::new();
    let result = to_mps::write_mps(&instance, &mut buffer);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), MpsWriteError::InvalidObjectiveType { degree: 2 }));
}

#[test]
fn test_nonlinear_constraint_error() {
    // Create instance with linear variable
    let mut instance = crate::v1::Instance::default();
    instance.decision_variables.push(crate::v1::DecisionVariable {
        id: 0,
        name: Some("x".to_string()),
        kind: crate::v1::decision_variable::Kind::Continuous as i32,
        bound: Some(crate::v1::Bound { lower: 0.0, upper: f64::INFINITY }),
        ..Default::default()
    });
    
    // Create constraint with quadratic function
    let mut func = crate::v1::Function::default();
    func.function = Some(crate::v1::function::Function::Quadratic(crate::v1::Quadratic {
        rows: vec![0],
        columns: vec![0],
        values: vec![1.0],
        linear: None,
    }));
    
    instance.constraints.push(crate::v1::Constraint {
        id: 0,
        name: Some("quad_constraint".to_string()),
        equality: crate::v1::Equality::LessThanOrEqualToZero as i32,
        function: Some(func),
        ..Default::default()
    });
    
    let mut buffer = Vec::new();
    let result = to_mps::write_mps(&instance, &mut buffer);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), MpsWriteError::InvalidConstraintType { name, degree: 2 } if name == "OMMX_CONSTR_0"));
}

#[test] 
fn test_cubic_polynomial_error() {
    // Test with degree 3 polynomial
    let mut instance = crate::v1::Instance::default();
    instance.decision_variables.push(crate::v1::DecisionVariable {
        id: 0,
        name: Some("x".to_string()),
        kind: crate::v1::decision_variable::Kind::Continuous as i32,
        bound: Some(crate::v1::Bound { lower: 0.0, upper: f64::INFINITY }),
        ..Default::default()
    });
    
    // Create a polynomial function (this would be degree 3 or higher)
    let mut func = crate::v1::Function::default();
    func.function = Some(crate::v1::function::Function::Polynomial(crate::v1::Polynomial {
        terms: vec![crate::v1::Monomial {
            coefficient: 1.0,
            ids: vec![0, 0, 0], // x^3 term
        }],
    }));
    
    instance.objective = Some(func);
    
    let mut buffer = Vec::new();
    let result = to_mps::write_mps(&instance, &mut buffer);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), MpsWriteError::InvalidObjectiveType { degree: 3 }));
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
    let result = to_mps::write_mps(&instance, &mut buffer);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(matches!(error, MpsWriteError::InvalidVariableId(var_id) if var_id == crate::VariableID::from(99)));
}

// Test write and read with compression
#[test]
fn test_write_file_compressed() {
    let instance = load_raw_reader(MPS_CONTENT.as_bytes()).unwrap();
    
    let temp_dir = TempDir::new("test_mps_write").unwrap();
    let compressed_path = temp_dir.path().join("test.mps.gz");
    let uncompressed_path = temp_dir.path().join("test.mps");
    
    // Write compressed
    write_file(&instance, &compressed_path, true).unwrap();
    assert!(compressed_path.exists());
    
    // Write uncompressed
    write_file(&instance, &uncompressed_path, false).unwrap();
    assert!(uncompressed_path.exists());
    
    // Both should load to same instance
    let from_compressed = load_file(&compressed_path).unwrap();
    let from_uncompressed = load_file(&uncompressed_path).unwrap();
    
    assert_eq!(from_compressed, from_uncompressed);
}
