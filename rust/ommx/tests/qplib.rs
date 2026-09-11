use ommx::{qplib, v1::State, ATol, ConstraintID, Evaluate};

const QUADRATIC_SCALING: &[u8] = include_bytes!("fixtures/quadratic_scaling.qplib");
const POINTS: [(f64, f64); 9] = [
    (0.0, 0.0),
    (1.0, 0.0),
    (-1.0, 0.0),
    (0.0, 1.0),
    (0.0, -1.0),
    (1.0, 1.0),
    (1.0, -1.0),
    (0.5, -0.75),
    (-1.0, 2.0),
];

#[test]
fn objective_preserves_diagonal_cross_linear_and_constant_terms() -> ommx::Result<()> {
    let instance = qplib::parse(QUADRATIC_SCALING)?;
    for (a, b) in POINTS {
        let state = State::from(std::collections::HashMap::from([(0, a), (1, b)]));
        let expected = a * a + 3.0 * a * b - 2.0 * b * b + 5.0 * a - 7.0 * b + 11.0;
        assert_eq!(
            instance.objective().evaluate(&state, ATol::default())?,
            expected
        );
    }
    Ok(())
}

#[test]
fn quadratic_constraint_preserves_both_bound_residuals() -> ommx::Result<()> {
    let instance = qplib::parse(QUADRATIC_SCALING)?;
    assert_eq!(instance.constraints().len(), 2);
    let upper = instance.constraints().get(&ConstraintID::from(0)).unwrap();
    let lower = instance.constraints().get(&ConstraintID::from(1)).unwrap();
    for (a, b) in POINTS {
        let state = State::from(std::collections::HashMap::from([(0, a), (1, b)]));
        let g = 2.0 * a * a - 5.0 * a * b + 3.0 * b * b + 13.0 * a + 17.0 * b;
        assert_eq!(
            upper.function().evaluate(&state, ATol::default())?,
            g - 23.0
        );
        assert_eq!(
            lower.function().evaluate(&state, ATol::default())?,
            -19.0 - g
        );
    }
    Ok(())
}

#[test]
fn published_solutions_preserve_objectives_and_feasibility() -> ommx::Result<()> {
    for (problem, solution, expected) in [
        (
            include_bytes!("fixtures/QPLIB_0018.qplib").as_slice(),
            include_bytes!("fixtures/QPLIB_0018.sol").as_slice(),
            -6.386_014_981_598_35,
        ),
        (
            include_bytes!("fixtures/QPLIB_0681.qplib").as_slice(),
            include_bytes!("fixtures/QPLIB_0681.sol").as_slice(),
            45.244_448_166_487_5,
        ),
    ] {
        let instance = qplib::parse(problem)?;
        let state = qplib::parse_solution(solution, instance.decision_variables().len())?;
        let solution = instance.evaluate(&state, ATol::new(1e-8)?)?;
        approx::assert_abs_diff_eq!(*solution.objective(), expected, epsilon = 1e-10);
        assert!(solution.feasible());
    }
    Ok(())
}

#[test]
fn solution_fills_zeroes_and_ignores_reported_objective() -> ommx::Result<()> {
    let state = qplib::parse_solution(
        b"# comment\n\nOBJVAR -1.2D+3\nx2 0.5\nB4 1\ni5 -2d-1\nx6 -0\n".as_slice(),
        6,
    )?;
    assert_eq!(state.entries.len(), 6);
    assert_eq!(state.entries[&0], 0.5);
    assert_eq!(state.entries[&1], 0.0);
    assert_eq!(state.entries[&2], 1.0);
    assert_eq!(state.entries[&3], -0.2);
    assert_eq!(state.entries[&4].to_bits(), 0.0_f64.to_bits());
    assert_eq!(state.entries[&5], 0.0);
    assert!(qplib::parse_solution(b"".as_slice(), 0)?.entries.is_empty());
    Ok(())
}

#[test]
fn solution_rejects_invalid_input_with_line_numbers() {
    for (input, line_num, message) in [
        (
            "\n# comment\nx2",
            3,
            "expected a variable name and a numeric value",
        ),
        ("x2 1 extra", 1, "expected exactly"),
        ("x2 invalid", 1, "invalid value"),
        ("x2 NaN", 1, "finite"),
        ("x2 inf", 1, "finite"),
        ("objvar -inf", 1, "finite"),
        ("x2 0\nx2 1", 2, "duplicate variable ID 0"),
        ("x2 1\nb2 1", 2, "duplicate variable ID 0"),
        ("objvar 0\nOBJVAR 0", 2, "duplicate objvar"),
        ("x1 1", 1, "invalid variable name"),
        ("b0 1", 1, "invalid variable name"),
        ("x 1", 1, "invalid variable name"),
        ("x+2 1", 1, "invalid variable name"),
        ("x18446744073709551616 1", 1, "invalid variable name"),
        ("custom_name 1", 1, "invalid variable name"),
        ("変数2 1", 1, "invalid variable name"),
        ("x5 1", 1, "outside 0..3"),
    ] {
        let error = qplib::parse_solution(input.as_bytes(), 3).unwrap_err();
        let signal = error.downcast_ref::<qplib::QplibParseError>().unwrap();
        assert_eq!(signal.line_num, line_num, "{input:?}");
        assert!(signal.message.contains(message), "{input:?}: {error}");
    }
}

#[test]
fn solution_propagates_read_failure_after_valid_prefix() {
    struct FailsAfterLine(bool);
    impl std::io::Read for FailsAfterLine {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.0 {
                return Err(std::io::Error::other("interrupted solution input"));
            }
            self.0 = true;
            let line = b"x2 1\n";
            buf[..line.len()].copy_from_slice(line);
            Ok(line.len())
        }
    }
    let error = qplib::parse_solution(FailsAfterLine(false), 1).unwrap_err();
    assert!(error.downcast_ref::<std::io::Error>().is_some());
    assert!(error.to_string().contains("line 2"));
}

proptest::proptest! {
    #[test]
    fn sparse_solution_text_preserves_generated_states(
        values in proptest::collection::vec(proptest::option::of(-1e6_f64..1e6_f64), 0..32),
    ) {
        let mut text = String::from("objvar 0\n");
        for (id, value) in values.iter().enumerate() {
            if let Some(value) = value {
                let prefix = ['x', 'b', 'i'][id % 3];
                text.push_str(&format!("{prefix}{} {value}\n", id + 2));
            }
        }
        let state = qplib::parse_solution(text.as_bytes(), values.len()).unwrap();
        proptest::prop_assert_eq!(state.entries.len(), values.len());
        for (id, value) in values.into_iter().enumerate() {
            proptest::prop_assert_eq!(state.entries[&(id as u64)], value.unwrap_or(0.0));
        }
    }
}
