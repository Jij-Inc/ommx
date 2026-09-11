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
fn objective_preserves_diagonal_cross_linear_and_constant_terms() -> anyhow::Result<()> {
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
fn quadratic_constraint_preserves_both_bound_residuals() -> anyhow::Result<()> {
    let instance = qplib::parse(QUADRATIC_SCALING)?;
    assert_eq!(instance.constraints().len(), 2);
    let upper = instance.constraints().get(&ConstraintID::from(0)).unwrap();
    let lower = instance.constraints().get(&ConstraintID::from(1)).unwrap();
    for (a, b) in POINTS {
        let state = State::from(std::collections::HashMap::from([(0, a), (1, b)]));
        let g = 2.0 * a * a - 5.0 * a * b + 3.0 * b * b + 13.0 * a + 17.0 * b;
        assert_eq!(upper.function.evaluate(&state, ATol::default())?, g - 23.0);
        assert_eq!(lower.function.evaluate(&state, ATol::default())?, -19.0 - g);
    }
    Ok(())
}
