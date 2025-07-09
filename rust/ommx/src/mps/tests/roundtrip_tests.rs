use super::*;
use crate::{random::InstanceParameters, v1::Instance};
use approx::AbsDiffEq;
use proptest::prelude::*;

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
        prop_assert!(instance.abs_diff_eq(&loaded_instance, crate::ATol::default()));
    }
}

// Test roundtrip for all test cases
#[test]
fn test_roundtrip_all_cases() {
    let test_cases = vec![
        ("basic", r#"NAME TestProblem
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
"#),
        ("complex", r#"NAME ComplexProblem
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
"#),
        ("integer", r#"NAME IntegerProblem
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
"#),
        ("binary", r#"NAME BinaryProblem
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
"#),
        ("free_var", r#"NAME FreeVarProblem
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
"#),
        ("maximize", r#"NAME MaximizeProblem
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
"#),
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