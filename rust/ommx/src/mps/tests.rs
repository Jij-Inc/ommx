// MPS module tests are now organized by purpose in separate files

mod compressed;
mod constraint_variable_tests;
mod read_tests;
mod roundtrip_tests;
mod write_tests;

// More complex MPS test case with multiple variables and constraints
pub const MPS_COMPLEX: &str = r#"NAME ComplexProblem
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
