use crate::{coeff, linear, mps::*, quadratic, Bound, Function};
use approx::assert_abs_diff_eq;

#[test]
fn test_neos_2626858_aoos_variable_domains() {
    use crate::decision_variable::Kind;

    let instance =
        parse(&include_bytes!("../../../tests/data/mps/neos-2626858-aoos.mps.gz")[..]).unwrap();
    let variables = instance.decision_variables();
    assert_eq!(variables.len(), 524);
    assert_eq!(instance.constraints().len(), 342);
    assert_eq!(
        variables
            .values()
            .filter(|v| v.kind() == Kind::Binary)
            .count(),
        209
    );
    assert_eq!(
        variables
            .values()
            .filter(|v| v.kind() == Kind::Integer)
            .count(),
        315
    );

    for index in (493..=508).chain([524]) {
        let name = format!("C{index:04}");
        let (_, variable) = variables
            .iter()
            .find(|(_, variable)| variable.metadata.name.as_deref() == Some(name.as_str()))
            .unwrap();
        assert_eq!(variable.kind(), Kind::Binary, "{name}");
        assert_eq!(variable.bound(), Bound::of_binary(), "{name}");
    }
}

#[test]
fn test_integer_marker_default_bounds() {
    use crate::decision_variable::Kind;

    // A bound record overrides the implicit binary domain, including LO/LI 0.
    // Integer declarations in BOUNDS alone do not use the marker default.
    for marked in [false, true] {
        for (bounds, lower, upper) in [
            ("", 0.0, if marked { 1.0 } else { f64::INFINITY }),
            ("BOUNDS\n", 0.0, if marked { 1.0 } else { f64::INFINITY }),
            ("BOUNDS\n LO BND X 0\n", 0.0, f64::INFINITY),
            ("BOUNDS\n LO BND X 2\n", 2.0, f64::INFINITY),
            ("BOUNDS\n LI BND X 0\n", 0.0, f64::INFINITY),
            ("BOUNDS\n LI BND X -2\n", -2.0, f64::INFINITY),
            ("BOUNDS\n UP BND X 5\n", 0.0, 5.0),
            ("BOUNDS\n UI BND X 5\n", 0.0, 5.0),
            ("BOUNDS\n PL BND X\n", 0.0, f64::INFINITY),
            ("BOUNDS\n MI BND X\n", f64::NEG_INFINITY, f64::INFINITY),
            ("BOUNDS\n FR BND X\n", f64::NEG_INFINITY, f64::INFINITY),
            ("BOUNDS\n FX BND X 3\n", 3.0, 3.0),
            ("BOUNDS\n BV BND X\n", 0.0, 1.0),
            ("BOUNDS\n UP BND X 1\n", 0.0, 1.0),
            ("BOUNDS\n LI BND X 0\n UI BND X 5\n", 0.0, 5.0),
            ("BOUNDS\n UI BND X 5\n LI BND X 0\n", 0.0, 5.0),
        ] {
            let (start, end) = if marked {
                (" MARK0 'MARKER' 'INTORG'\n", " MARK1 'MARKER' 'INTEND'\n")
            } else {
                ("", "")
            };
            let input = format!(
                "NAME MarkerBounds\nROWS\n N OBJ\nCOLUMNS\n{start} X OBJ 1\n{end}{bounds}ENDATA\n"
            );
            let instance = parse(input.as_bytes()).unwrap();
            let variable = instance.decision_variables().values().next().unwrap();
            let integer = marked || bounds.contains(" LI ") || bounds.contains(" UI ");
            let kind = if bounds.contains(" BV ") || (integer && lower == 0.0 && upper == 1.0) {
                Kind::Binary
            } else if integer {
                Kind::Integer
            } else {
                Kind::Continuous
            };
            assert_eq!(variable.kind(), kind, "{input}");
            assert_eq!(
                variable.bound(),
                Bound::new(lower, upper).unwrap(),
                "{input}"
            );
        }
    }
}

#[test]
fn test_integer_markers_with_mixed_columns() {
    let input = indoc::indoc! {"
        NAME MixedMarkers
        ROWS
         N OBJ
         L ROW
        COLUMNS
         BEFORE OBJ 1
         MARK0 'MARKER' 'INTORG'
         BINARY1 OBJ 1
         BINARY1 ROW 1
         INTEGER OBJ 1
         MARK1 'MARKER' 'INTEND'
         BETWEEN OBJ 1
         MARK2 'MARKER' 'INTORG'
         BINARY2 OBJ 1
         MARK3 'MARKER' 'INTEND'
         AFTER OBJ 1
        BOUNDS
         UP BND INTEGER 5
        ENDATA
    "};
    let instance = parse(input.as_bytes()).unwrap();
    use crate::decision_variable::Kind::{Binary, Continuous, Integer};
    let expected = [
        (Continuous, f64::INFINITY),
        (Binary, 1.0),
        (Integer, 5.0),
        (Continuous, f64::INFINITY),
        (Binary, 1.0),
        (Continuous, f64::INFINITY),
    ];
    assert_eq!(instance.decision_variables().len(), expected.len());
    for (variable, (kind, upper)) in instance.decision_variables().values().zip(expected) {
        assert_eq!(variable.kind(), kind);
        assert_eq!(variable.bound(), Bound::new(0.0, upper).unwrap());
    }
}

// Test basic MPS parsing
#[test]
fn test_basic_mps_parsing() {
    // minimize x1
    // s.t.     x1 <= 5
    //          x1 \in [0, 4]
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

    let instance = parse(MPS_CONTENT.as_bytes()).unwrap();

    // Default sense is Minimize
    assert_eq!(instance.sense(), crate::Sense::Minimize);

    // Check variable
    assert_eq!(instance.decision_variables().len(), 1);
    let (_, var) = instance.decision_variables().iter().next().unwrap();
    assert_eq!(var.metadata.name.as_ref().unwrap(), "X1");
    assert_eq!(var.kind(), crate::decision_variable::Kind::Continuous);
    assert_eq!(var.bound(), Bound::new(0.0, 4.0).unwrap());

    // Check objective: x1
    assert_abs_diff_eq!(instance.objective(), &Function::from(linear!(var.id())));

    // Check constraint: x1 - 5 <= 0
    assert_eq!(instance.constraints().len(), 1);
    let (_, constraint) = instance.constraints().iter().next().unwrap();
    assert_abs_diff_eq!(
        &constraint.function,
        &Function::from(linear!(var.id()) + coeff!(-5.0))
    );
    assert_eq!(constraint.equality, crate::Equality::LessThanOrEqualToZero);
    assert_eq!(constraint.name.as_ref().unwrap(), "R1");
}

// Test MPS with RANGES section
#[test]
fn test_mps_with_ranges() {
    // minimize x1 + 2*x2
    // s.t. R1: 10-2 <= x1 + 2*x2 <= 10
    //      R2:    5 <= x1 + x2   <= 5+3
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

    let instance = parse(MPS_WITH_RANGES.as_bytes()).unwrap();

    // Check basic structure
    assert_eq!(instance.decision_variables().len(), 2);
    let mut iter = instance.decision_variables().values();
    let x1 = iter.next().unwrap();
    let x2 = iter.next().unwrap();
    // Default kind is Continuous
    assert_eq!(x1.kind(), crate::decision_variable::Kind::Continuous);
    assert_eq!(x2.kind(), crate::decision_variable::Kind::Continuous);
    // Default bound is [0, ∞)
    assert_eq!(x1.bound(), Bound::new(0.0, f64::INFINITY).unwrap());
    assert_eq!(x2.bound(), Bound::new(0.0, f64::INFINITY).unwrap());
    // Check variable names
    assert_eq!(x1.metadata.name.as_ref().unwrap(), "X1");
    assert_eq!(x2.metadata.name.as_ref().unwrap(), "X2");

    // Check objective: x1 + 2*x2
    assert_abs_diff_eq!(
        instance.objective(),
        &Function::from(linear!(x1.id()) + coeff!(2.0) * linear!(x2.id()))
    );

    // Check constraints
    assert_eq!(instance.constraints().len(), 4);
    let mut iter = instance.constraints().values();
    let r1 = iter.next().unwrap();
    let r2 = iter.next().unwrap();
    let r1_range = iter.next().unwrap();
    let r2_range = iter.next().unwrap();
    // x1 + 2*x2 - 10 <= 0
    assert_abs_diff_eq!(
        &r1.function,
        &Function::from(linear!(x1.id()) + coeff!(2.0) * linear!(x2.id()) + coeff!(-10.0))
    );
    assert_eq!(r1.equality, crate::Equality::LessThanOrEqualToZero);
    assert_eq!(r1.name.as_ref().unwrap(), "R1");
    // -x1 - x2 + 5 <= 0
    assert_abs_diff_eq!(
        &r2.function,
        &Function::from(-linear!(x1.id()) - linear!(x2.id()) + coeff!(5.0))
    );
    assert_eq!(r2.equality, crate::Equality::LessThanOrEqualToZero);
    assert_eq!(r2.name.as_ref().unwrap(), "R2");
    // -x1 - 2*x2 + 8 <= 0
    assert_abs_diff_eq!(
        &r1_range.function,
        &Function::from(-linear!(x1.id()) - coeff!(2.0) * linear!(x2.id()) + coeff!(8.0))
    );
    assert_eq!(r1_range.equality, crate::Equality::LessThanOrEqualToZero);
    assert_eq!(r1_range.name.as_ref().unwrap(), "R1_");
    // x1 + x2 - 8 <= 0
    assert_abs_diff_eq!(
        &r2_range.function,
        &Function::from(linear!(x1.id()) + linear!(x2.id()) + coeff!(-8.0))
    );
    assert_eq!(r2_range.equality, crate::Equality::LessThanOrEqualToZero);
    assert_eq!(r2_range.name.as_ref().unwrap(), "R2_");
}

// Test integer variables
#[test]
fn test_integer_variables() {
    // minimize x1 + 2*x2 + 3*x3
    // s.t.     x1 + x2 + x3 <= 10
    //          x1, x2, x3 \in [0, 5]
    //          x1, x2 \in Z
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

    let instance = parse(MPS_INTEGER.as_bytes()).unwrap();

    // The IDs of the decision variables are registered in the order they appear
    assert_eq!(instance.decision_variables().len(), 3);
    let mut iter = instance.decision_variables().values();
    let x1 = iter.next().unwrap();
    let x2 = iter.next().unwrap();
    let x3 = iter.next().unwrap();

    assert_eq!(x1.kind(), crate::decision_variable::Kind::Integer);
    assert_eq!(x2.kind(), crate::decision_variable::Kind::Integer);
    assert_eq!(x3.kind(), crate::decision_variable::Kind::Continuous);
    assert_eq!(x1.bound(), Bound::new(0.0, 5.0).unwrap());
    assert_eq!(x2.bound(), Bound::new(0.0, 5.0).unwrap());
    assert_eq!(x3.bound(), Bound::new(0.0, 5.0).unwrap());
    // Check variable names
    assert_eq!(x1.metadata.name.as_ref().unwrap(), "X1");
    assert_eq!(x2.metadata.name.as_ref().unwrap(), "X2");
    assert_eq!(x3.metadata.name.as_ref().unwrap(), "X3");

    // Check objective: x1 + 2*x2 + 3*x3
    assert_abs_diff_eq!(
        instance.objective(),
        &Function::from(
            linear!(x1.id()) + coeff!(2.0) * linear!(x2.id()) + coeff!(3.0) * linear!(x3.id())
        )
    );

    // Check constraint: x1 + x2 + x3 - 10 <= 0
    assert_eq!(instance.constraints().len(), 1);
    let (_, constraint) = instance.constraints().iter().next().unwrap();
    assert_abs_diff_eq!(
        &constraint.function,
        &Function::from(linear!(x1.id()) + linear!(x2.id()) + linear!(x3.id()) + coeff!(-10.0))
    );
    assert_eq!(constraint.equality, crate::Equality::LessThanOrEqualToZero);
    assert_eq!(constraint.name.as_ref().unwrap(), "C1");
}

// Test binary variables
#[test]
fn test_binary_variables() {
    // minimize x1 + 2*x2
    // s.t.     x1 + x2 <= 1
    //          x1, x2 \in {0, 1}
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

    let instance = parse(MPS_BINARY.as_bytes()).unwrap();

    // Check variables
    assert_eq!(instance.decision_variables().len(), 2);
    let mut iter = instance.decision_variables().values();
    let x1 = iter.next().unwrap();
    let x2 = iter.next().unwrap();

    // Both should be binary
    assert_eq!(x1.kind(), crate::decision_variable::Kind::Binary);
    assert_eq!(x2.kind(), crate::decision_variable::Kind::Binary);
    // Check variable names
    assert_eq!(x1.metadata.name.as_ref().unwrap(), "X1");
    assert_eq!(x2.metadata.name.as_ref().unwrap(), "X2");

    // Check objective: x1 + 2*x2
    assert_abs_diff_eq!(
        instance.objective(),
        &Function::from(linear!(x1.id()) + coeff!(2.0) * linear!(x2.id()))
    );

    // Check constraint: x1 + x2 - 1 <= 0
    assert_eq!(instance.constraints().len(), 1);
    let (_, constraint) = instance.constraints().iter().next().unwrap();
    assert_abs_diff_eq!(
        &constraint.function,
        &Function::from(linear!(x1.id()) + linear!(x2.id()) + coeff!(-1.0))
    );
    assert_eq!(constraint.equality, crate::Equality::LessThanOrEqualToZero);
    assert_eq!(constraint.name.as_ref().unwrap(), "C1");
}

// Test free variables
#[test]
fn test_free_variables() {
    // minimize x1 - x2
    // s.t.     x1 - x2 = 0
    //          x1, x2 \in (-∞, ∞)
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

    let instance = parse(MPS_FREE_VAR.as_bytes()).unwrap();

    // Check variables
    assert_eq!(instance.decision_variables().len(), 2);
    let mut iter = instance.decision_variables().values();
    let x1 = iter.next().unwrap();
    let x2 = iter.next().unwrap();
    assert_eq!(x1.kind(), crate::decision_variable::Kind::Continuous);
    assert_eq!(x2.kind(), crate::decision_variable::Kind::Continuous);
    assert_eq!(
        x1.bound(),
        Bound::new(f64::NEG_INFINITY, f64::INFINITY).unwrap()
    );
    assert_eq!(
        x2.bound(),
        Bound::new(f64::NEG_INFINITY, f64::INFINITY).unwrap()
    );
    // Check variable names
    assert_eq!(x1.metadata.name.as_ref().unwrap(), "X1");
    assert_eq!(x2.metadata.name.as_ref().unwrap(), "X2");

    // Check objective: x1 - x2
    assert_abs_diff_eq!(
        instance.objective(),
        &Function::from(linear!(x1.id()) + coeff!(-1.0) * linear!(x2.id()))
    );

    // Check constraint: x1 - x2 = 0
    assert_eq!(instance.constraints().len(), 1);
    let (_, constraint) = instance.constraints().iter().next().unwrap();
    assert_abs_diff_eq!(
        &constraint.function,
        &Function::from(linear!(x1.id()) + coeff!(-1.0) * linear!(x2.id()))
    );
    assert_eq!(constraint.equality, crate::Equality::EqualToZero);
    assert_eq!(constraint.name.as_ref().unwrap(), "C1");
}

// Test OBJSENSE
#[test]
fn test_objsense_maximize() {
    // maximize x1 + 2*x2
    // s.t.     x1 + x2 <= 10
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

    let instance = parse(MPS_MAXIMIZE.as_bytes()).unwrap();

    // Check sense
    assert_eq!(instance.sense(), crate::Sense::Maximize);

    // Check variables
    assert_eq!(instance.decision_variables().len(), 2);
    let mut iter = instance.decision_variables().values();
    let x1 = iter.next().unwrap();
    let x2 = iter.next().unwrap();
    // Check variable names
    assert_eq!(x1.metadata.name.as_ref().unwrap(), "X1");
    assert_eq!(x2.metadata.name.as_ref().unwrap(), "X2");

    // Check objective: x1 + 2*x2
    assert_abs_diff_eq!(
        instance.objective(),
        &Function::from(linear!(x1.id()) + coeff!(2.0) * linear!(x2.id()))
    );

    // Check constraint: x1 + x2 - 10 <= 0
    assert_eq!(instance.constraints().len(), 1);
    let (_, constraint) = instance.constraints().iter().next().unwrap();
    assert_abs_diff_eq!(
        &constraint.function,
        &Function::from(linear!(x1.id()) + linear!(x2.id()) + coeff!(-10.0))
    );
    assert_eq!(constraint.equality, crate::Equality::LessThanOrEqualToZero);
    assert_eq!(constraint.name.as_ref().unwrap(), "C1");
}

// Test complex MPS with all constraint types
#[test]
fn test_complex_mps_parsing() {
    // minimize x1 + 4*x2 + 9*x3
    // s.t.     2*x1 + x2 <= 5        (C1)
    //          x1 + x3 >= 10          (C2)
    //          x1 - x2 + x3 = 7       (C3)
    //          x1 \in [0, 4]
    //          x2 \in [-1, 1]
    //          x3 \in [0, ∞)
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

    let instance = parse(MPS_COMPLEX.as_bytes()).unwrap();

    // Check variables
    assert_eq!(instance.decision_variables().len(), 3);
    let mut iter = instance.decision_variables().values();
    let x1 = iter.next().unwrap();
    let x2 = iter.next().unwrap();
    let x3 = iter.next().unwrap();

    // Check bounds
    assert_eq!(x1.bound(), Bound::new(0.0, 4.0).unwrap());
    assert_eq!(x2.bound(), Bound::new(-1.0, 1.0).unwrap());
    assert_eq!(x3.bound(), Bound::new(0.0, f64::INFINITY).unwrap());
    // Check variable names
    assert_eq!(x1.metadata.name.as_ref().unwrap(), "X1");
    assert_eq!(x2.metadata.name.as_ref().unwrap(), "X2");
    assert_eq!(x3.metadata.name.as_ref().unwrap(), "X3");

    // Check objective: x1 + 4*x2 + 9*x3
    assert_abs_diff_eq!(
        instance.objective(),
        &Function::from(
            linear!(x1.id()) + coeff!(4.0) * linear!(x2.id()) + coeff!(9.0) * linear!(x3.id())
        )
    );

    // Check constraints
    assert_eq!(instance.constraints().len(), 3);

    // Check that all variables have valid bounds
    for var in instance.decision_variables().values() {
        let bound = var.bound();
        assert!(bound.lower() <= bound.upper());
    }

    // Check that all constraints have valid equality types
    for constraint in instance.constraints().values() {
        match constraint.equality {
            crate::Equality::EqualToZero | crate::Equality::LessThanOrEqualToZero => {
                // These are expected
            }
        }
    }
}

// Test MPS parsing with QUADOBJ section
#[test]
fn test_parse_quadobj() {
    let mps_content = r#"NAME QUADTEST
ROWS
 N  OBJ
 L  CON1
COLUMNS
    X1        OBJ                              1
    X1        CON1                             2
    X2        OBJ                              3
    X2        CON1                             4
RHS
    RHS1      CON1                            10
QUADOBJ
    X1        X1                             0.5
    X1        X2                             1.0
    X2        X2                             2.0
ENDATA
"#;

    let instance = parse(mps_content.as_bytes()).unwrap();

    // Check that we have 2 variables
    assert_eq!(instance.decision_variables().len(), 2);

    // Check that we have 1 constraint
    assert_eq!(instance.constraints().len(), 1);

    // The objective should be quadratic
    assert_eq!(instance.objective().degree(), 2);

    // Get variables using the new helper method
    let x1 = instance
        .get_decision_variable_by_name("X1", vec![])
        .unwrap();
    let x2 = instance
        .get_decision_variable_by_name("X2", vec![])
        .unwrap();
    let x1_id = x1.id();
    let x2_id = x2.id();

    // Build expected objective function: x1 + 3*x2 + 0.5*x1^2 + x1*x2 + 2*x2^2
    let expected_objective = quadratic!(x1_id)
        + coeff!(3.0) * quadratic!(x2_id)
        + coeff!(0.5) * quadratic!(x1_id, x1_id)
        + quadratic!(x1_id, x2_id)
        + coeff!(2.0) * quadratic!(x2_id, x2_id);

    // Compare the actual and expected objective functions
    assert_abs_diff_eq!(instance.objective(), &expected_objective.into());
}

// Test MPS parsing with QCMATRIX section
#[test]
fn test_parse_qcmatrix() {
    let mps_content = r#"NAME QUADTEST
ROWS
 N  OBJ
 L  CON1
COLUMNS
    X1        OBJ                              1
    X1        CON1                             2
    X2        OBJ                              3
    X2        CON1                             4
RHS
    RHS1      CON1                            10
QCMATRIX CON1
    X1        X1                             0.5
    X1        X2                             1.0
ENDATA
"#;

    let instance = parse(mps_content.as_bytes()).unwrap();

    // Check that we have 2 variables
    assert_eq!(instance.decision_variables().len(), 2);

    // Check that we have 1 constraint
    assert_eq!(instance.constraints().len(), 1);

    // Get variables using the helper method
    let x1 = instance
        .get_decision_variable_by_name("X1", vec![])
        .unwrap();
    let x2 = instance
        .get_decision_variable_by_name("X2", vec![])
        .unwrap();
    let x1_id = x1.id();
    let x2_id = x2.id();

    // The constraint should be quadratic
    let (_, constraint) = instance.constraints().iter().next().unwrap();
    assert_eq!(constraint.function.degree(), 2);

    // Build expected constraint function: 2*x1 + 4*x2 + 0.5*x1^2 + x1*x2 - 10 <= 0
    // Note: RHS is moved to LHS, so the constant term is -10
    let expected_function = coeff!(2.0) * quadratic!(x1_id)
        + coeff!(4.0) * quadratic!(x2_id)
        + coeff!(0.5) * quadratic!(x1_id, x1_id)
        + quadratic!(x1_id, x2_id)
        + coeff!(-10.0);

    // Compare the actual and expected constraint functions
    assert_abs_diff_eq!(&constraint.function, &expected_function.into());

    // Verify it's a less-than-or-equal constraint
    assert_eq!(constraint.equality, crate::Equality::LessThanOrEqualToZero);
}
