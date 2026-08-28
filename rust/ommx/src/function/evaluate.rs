use super::operation::{
    as_constant, from_instructions_exact, instructions, into_instructions, AssociativeOperator,
    Atom, BinaryOperator, Instruction, UnaryOperator,
};
use super::*;
use crate::{Evaluate, Sampled, VariableIDSet};

fn ensure_finite(operation: &'static str, value: f64) -> crate::Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(FunctionEvaluationError::NonFiniteOperand { operation, value }.into())
    }
}

fn ensure_finite_result(operation: &'static str, value: f64) -> crate::Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(FunctionEvaluationError::NonFiniteResult { operation }.into())
    }
}

fn ensure_finite_sampled_results(
    operation: &'static str,
    mut values: Sampled<f64>,
) -> crate::Result<Sampled<f64>> {
    for value in values.iter_mut() {
        ensure_finite_result(operation, *value)?;
    }
    Ok(values)
}

fn ensure_required_state_entries(
    required_ids: VariableIDSet,
    state: &crate::v1::State,
) -> crate::Result<()> {
    let missing_ids = required_ids
        .into_iter()
        .filter(|id| !state.entries.contains_key(&id.into_inner()))
        .collect::<VariableIDSet>();
    if missing_ids.is_empty() {
        Ok(())
    } else {
        Err(crate::MissingStateEntries { ids: missing_ids }.into())
    }
}

fn is_zero(value: f64, atol: crate::ATol) -> bool {
    value.abs() <= *atol
}

fn unary_partial_fold_is_atol_independent(operator: UnaryOperator) -> bool {
    match operator {
        UnaryOperator::Neg | UnaryOperator::Abs => true,
        UnaryOperator::Powi(exponent) => exponent >= 0,
        UnaryOperator::Signum => false,
    }
}

fn polynomial_has_state_overlap<M: crate::Monomial>(
    function: &crate::PolynomialBase<M>,
    state: &crate::v1::State,
) -> bool {
    function.keys().any(|monomial| {
        monomial
            .ids()
            .any(|id| state.entries.contains_key(&id.into_inner()))
    })
}

fn expression_has_partial_fold_candidate(expression: &Expression) -> bool {
    let mut constant_operands = Vec::new();
    for instruction in instructions(expression) {
        match instruction {
            Instruction::Push(Atom::Zero | Atom::Constant(_)) => constant_operands.push(true),
            Instruction::Push(_) => constant_operands.push(false),
            Instruction::Unary(operator) => {
                let operand = constant_operands
                    .pop()
                    .expect("validated unary instruction has an operand");
                if operand && unary_partial_fold_is_atol_independent(*operator) {
                    return true;
                }
                constant_operands.push(false);
            }
            Instruction::Associative(_) => {
                let rhs = constant_operands
                    .pop()
                    .expect("validated associative instruction has a right operand");
                let lhs = constant_operands
                    .pop()
                    .expect("validated associative instruction has a left operand");
                if lhs && rhs {
                    return true;
                }
                constant_operands.push(false);
            }
            Instruction::Binary(_) => {
                constant_operands
                    .pop()
                    .expect("validated binary instruction has a right operand");
                constant_operands
                    .pop()
                    .expect("validated binary instruction has a left operand");
                constant_operands.push(false);
            }
        }
    }
    false
}

fn expression_needs_partial_rebuild(expression: &Expression, state: &crate::v1::State) -> bool {
    instructions(expression).iter().any(|instruction| {
        let Instruction::Push(atom) = instruction else {
            return false;
        };
        match atom {
            Atom::Linear(function) => {
                polynomial_has_state_overlap(function, state) || function.degree().into_inner() != 1
            }
            Atom::Quadratic(function) => {
                polynomial_has_state_overlap(function, state) || function.degree().into_inner() != 2
            }
            Atom::Polynomial(function) => {
                polynomial_has_state_overlap(function, state) || function.degree().into_inner() <= 2
            }
            Atom::Zero | Atom::Constant(_) => false,
        }
    }) || expression_has_partial_fold_candidate(expression)
}

fn canonicalize_borrowed_linear(function: &crate::Linear) -> Function {
    if function.is_zero() {
        Function::Zero
    } else {
        Function::Constant(
            function
                .get(&crate::LinearMonomial::Constant)
                .expect("non-zero degree-0 linear has a constant term"),
        )
    }
}

fn canonicalize_borrowed_quadratic(function: &crate::Quadratic) -> Function {
    if function.is_zero() {
        return Function::Zero;
    }
    match function.degree().into_inner() {
        1 => Function::Linear(
            crate::Linear::try_from(function).expect("degree-1 quadratic is linear"),
        ),
        0 => Function::Constant(
            function
                .get(&crate::QuadraticMonomial::Constant)
                .expect("non-zero degree-0 quadratic has a constant term"),
        ),
        _ => unreachable!("canonical quadratic replacements have degree below two"),
    }
}

fn canonicalize_borrowed_polynomial(function: &crate::Polynomial) -> Function {
    if function.is_zero() {
        return Function::Zero;
    }
    match function.degree().into_inner() {
        2 => Function::Quadratic(
            crate::Quadratic::try_from(function).expect("degree-2 polynomial is quadratic"),
        ),
        1 => Function::Linear(
            crate::Linear::try_from(function).expect("degree-1 polynomial is linear"),
        ),
        0 => Function::Constant(
            function
                .get(&crate::MonomialDyn::default())
                .expect("non-zero degree-0 polynomial has a constant term"),
        ),
        _ => unreachable!("canonical polynomial replacements have degree at most two"),
    }
}

impl Function {
    /// Prepare a replacement for the `Instance`-owned atomic plan without
    /// mutating or cloning a function that needs no substitution,
    /// canonicalization, or closed-expression folding.
    pub(crate) fn partial_evaluate_replacement(
        &self,
        state: &crate::v1::State,
        atol: crate::ATol,
    ) -> crate::Result<Option<Self>> {
        Ok(match self {
            Function::Zero | Function::Constant(_) => None,
            Function::Linear(function) => match function.partial_evaluate_replacement(state)? {
                Some(function) => Some(Function::Linear(function).normalize()),
                None if function.degree().into_inner() != 1 => {
                    Some(canonicalize_borrowed_linear(function))
                }
                None => None,
            },
            Function::Quadratic(function) => match function.partial_evaluate_replacement(state)? {
                Some(function) => Some(Function::Quadratic(function).normalize()),
                None if function.degree().into_inner() != 2 => {
                    Some(canonicalize_borrowed_quadratic(function))
                }
                None => None,
            },
            Function::Polynomial(function) => match function.partial_evaluate_replacement(state)? {
                Some(function) => Some(Function::Polynomial(function).normalize()),
                None if function.degree().into_inner() <= 2 => {
                    Some(canonicalize_borrowed_polynomial(function))
                }
                None => None,
            },
            Function::Expression(expression) => {
                if expression_needs_partial_rebuild(expression, state) {
                    Some(partially_evaluate_expression(expression, state, atol)?)
                } else {
                    None
                }
            }
        })
    }
}

fn evaluate_atom(
    atom: &Atom,
    solution: &crate::v1::State,
    atol: crate::ATol,
) -> crate::Result<f64> {
    let value = match atom {
        Atom::Zero => 0.0,
        Atom::Constant(c) => c.into_inner(),
        Atom::Linear(f) => f.evaluate(solution, atol)?,
        Atom::Quadratic(f) => f.evaluate(solution, atol)?,
        Atom::Polynomial(f) => f.evaluate(solution, atol)?,
    };
    ensure_finite_result("polynomial function", value)
}

fn evaluate_unary(operator: UnaryOperator, operand: f64, atol: crate::ATol) -> crate::Result<f64> {
    let operand = ensure_finite("unary operation operand", operand)?;
    let value = match operator {
        UnaryOperator::Neg => -operand,
        UnaryOperator::Abs => operand.abs(),
        UnaryOperator::Signum if is_zero(operand, atol) => 0.0,
        UnaryOperator::Signum if operand < 0.0 => -1.0,
        UnaryOperator::Signum => 1.0,
        UnaryOperator::Powi(exponent) if exponent < 0 && is_zero(operand, atol) => {
            return Err(FunctionEvaluationError::ZeroToNegativeIntegerPower { exponent }.into());
        }
        UnaryOperator::Powi(exponent) => operand.powi(exponent),
    };
    ensure_finite_result("unary operation", value)
}

fn evaluate_associative(operator: AssociativeOperator, lhs: f64, rhs: f64) -> crate::Result<f64> {
    let lhs = ensure_finite("associative operation operand", lhs)?;
    let rhs = ensure_finite("associative operation operand", rhs)?;
    let value = match operator {
        AssociativeOperator::Add => lhs + rhs,
        AssociativeOperator::Mul => lhs * rhs,
        AssociativeOperator::Min => lhs.min(rhs),
        AssociativeOperator::Max => lhs.max(rhs),
    };
    ensure_finite_result("associative operation", value)
}

fn evaluate_binary(
    operator: BinaryOperator,
    lhs: f64,
    rhs: f64,
    atol: crate::ATol,
) -> crate::Result<f64> {
    let lhs = ensure_finite("binary operation left operand", lhs)?;
    let rhs = ensure_finite("binary operation right operand", rhs)?;
    let value = match operator {
        BinaryOperator::Div if is_zero(rhs, atol) => {
            return Err(FunctionEvaluationError::DivisionByZero.into());
        }
        BinaryOperator::Div => lhs / rhs,
    };
    ensure_finite_result("binary operation", value)
}

#[derive(Clone, Copy)]
struct PartialOperand {
    start: usize,
    /// A value is present when this operand can be folded without an `atol`-dependent decision.
    value: Option<f64>,
}

fn replace_partial_operand_with_value(
    instructions: &mut Vec<Instruction>,
    start: usize,
    value: f64,
) -> PartialOperand {
    instructions.truncate(start);
    let constant = Function::try_from(value)
        .expect("successful Function evaluation always returns a finite value");
    instructions.extend(into_instructions(constant));
    PartialOperand {
        start,
        value: Some(value),
    }
}

fn partially_evaluate_expression(
    expression: &Expression,
    state: &crate::v1::State,
    atol: crate::ATol,
) -> crate::Result<Function> {
    let mut output_instructions = Vec::with_capacity(instructions(expression).len());
    let mut operands = Vec::new();

    for instruction in instructions(expression) {
        match instruction {
            Instruction::Push(atom) => {
                let evaluated = match atom {
                    Atom::Zero => Function::Zero,
                    Atom::Constant(value) => Function::Constant(value.to_owned()),
                    Atom::Linear(function) => Function::Linear(
                        function
                            .partial_evaluate_replacement(state)?
                            .unwrap_or_else(|| function.clone()),
                    )
                    .normalize(),
                    Atom::Quadratic(function) => Function::Quadratic(
                        function
                            .partial_evaluate_replacement(state)?
                            .unwrap_or_else(|| function.clone()),
                    )
                    .normalize(),
                    Atom::Polynomial(function) => Function::Polynomial(
                        function
                            .partial_evaluate_replacement(state)?
                            .unwrap_or_else(|| function.clone()),
                    )
                    .normalize(),
                };
                let value = as_constant(&evaluated);
                debug_assert_eq!(value.is_some(), evaluated.required_ids().is_empty());
                let start = output_instructions.len();
                output_instructions.extend(into_instructions(evaluated));
                operands.push(PartialOperand { start, value });
            }
            Instruction::Unary(operator) => {
                let operand = operands
                    .pop()
                    .expect("validated unary instruction has an operand");
                if let Some(value) = operand
                    .value
                    .filter(|_| unary_partial_fold_is_atol_independent(*operator))
                {
                    let value = evaluate_unary(*operator, value, atol)?;
                    operands.push(replace_partial_operand_with_value(
                        &mut output_instructions,
                        operand.start,
                        value,
                    ));
                } else {
                    output_instructions.push(Instruction::Unary(*operator));
                    operands.push(PartialOperand {
                        start: operand.start,
                        value: None,
                    });
                }
            }
            Instruction::Associative(operator) => {
                let rhs = operands
                    .pop()
                    .expect("validated associative instruction has a right operand");
                let lhs = operands
                    .pop()
                    .expect("validated associative instruction has a left operand");
                if let (Some(lhs_value), Some(rhs_value)) = (lhs.value, rhs.value) {
                    let value = evaluate_associative(*operator, lhs_value, rhs_value)?;
                    operands.push(replace_partial_operand_with_value(
                        &mut output_instructions,
                        lhs.start,
                        value,
                    ));
                } else {
                    output_instructions.push(Instruction::Associative(*operator));
                    operands.push(PartialOperand {
                        start: lhs.start,
                        value: None,
                    });
                }
            }
            Instruction::Binary(operator) => {
                let _rhs = operands
                    .pop()
                    .expect("validated binary instruction has a right operand");
                let lhs = operands
                    .pop()
                    .expect("validated binary instruction has a left operand");
                output_instructions.push(Instruction::Binary(*operator));
                operands.push(PartialOperand {
                    start: lhs.start,
                    value: None,
                });
            }
        }
    }

    debug_assert_eq!(operands.len(), 1);
    Ok(from_instructions_exact(output_instructions)
        .expect("folding closed operands preserves expression validity"))
}

impl Evaluate for Function {
    type Output = f64;
    type SampledOutput = Sampled<f64>;

    fn evaluate(
        &self,
        solution: &crate::v1::State,
        atol: crate::ATol,
    ) -> crate::Result<Self::Output> {
        match self {
            Function::Zero => Ok(0.0),
            Function::Constant(c) => Ok(c.into_inner()),
            Function::Linear(f) => {
                ensure_finite_result("polynomial function", f.evaluate(solution, atol)?)
            }
            Function::Quadratic(f) => {
                ensure_finite_result("polynomial function", f.evaluate(solution, atol)?)
            }
            Function::Polynomial(f) => {
                ensure_finite_result("polynomial function", f.evaluate(solution, atol)?)
            }
            Function::Expression(expression) => {
                ensure_required_state_entries(self.required_ids(), solution)?;
                let mut values = Vec::new();
                for instruction in instructions(expression) {
                    match instruction {
                        Instruction::Push(atom) => {
                            values.push(evaluate_atom(atom, solution, atol)?)
                        }
                        Instruction::Unary(operator) => {
                            let operand = values
                                .pop()
                                .expect("validated unary instruction has an operand");
                            values.push(evaluate_unary(*operator, operand, atol)?);
                        }
                        Instruction::Associative(operator) => {
                            let rhs = values
                                .pop()
                                .expect("validated associative instruction has a right operand");
                            let lhs = values
                                .pop()
                                .expect("validated associative instruction has a left operand");
                            values.push(evaluate_associative(*operator, lhs, rhs)?);
                        }
                        Instruction::Binary(operator) => {
                            let rhs = values
                                .pop()
                                .expect("validated binary instruction has a right operand");
                            let lhs = values
                                .pop()
                                .expect("validated binary instruction has a left operand");
                            values.push(evaluate_binary(*operator, lhs, rhs, atol)?);
                        }
                    }
                }
                Ok(values
                    .pop()
                    .expect("validated expression leaves one result"))
            }
        }
    }

    fn partial_evaluate(
        &mut self,
        state: &crate::v1::State,
        atol: crate::ATol,
    ) -> crate::Result<()> {
        match self {
            Function::Zero | Function::Constant(_) => return Ok(()),
            Function::Linear(function) => function.partial_evaluate(state, atol)?,
            Function::Quadratic(function) => function.partial_evaluate(state, atol)?,
            Function::Polynomial(function) => function.partial_evaluate(state, atol)?,
            Function::Expression(expression) => {
                if !expression_needs_partial_rebuild(expression, state) {
                    return Ok(());
                }
                let evaluated = partially_evaluate_expression(expression, state, atol)?;
                *self = evaluated;
                return Ok(());
            }
        }

        *self = std::mem::take(self).normalize();
        Ok(())
    }

    fn required_ids(&self) -> VariableIDSet {
        match self {
            Function::Linear(f) => f.required_ids(),
            Function::Quadratic(f) => f.required_ids(),
            Function::Polynomial(f) => f.required_ids(),
            Function::Expression(expression) => {
                let mut ids = VariableIDSet::default();
                for instruction in instructions(expression) {
                    let Instruction::Push(atom) = instruction else {
                        continue;
                    };
                    match atom {
                        Atom::Linear(f) => ids.extend(f.required_ids()),
                        Atom::Quadratic(f) => ids.extend(f.required_ids()),
                        Atom::Polynomial(f) => ids.extend(f.required_ids()),
                        Atom::Zero | Atom::Constant(_) => {}
                    }
                }
                ids
            }
            Function::Zero | Function::Constant(_) => VariableIDSet::default(),
        }
    }

    fn evaluate_samples(
        &self,
        samples: &Sampled<crate::v1::State>,
        atol: crate::ATol,
    ) -> crate::Result<Self::SampledOutput> {
        match self {
            Function::Zero => Ok(Sampled::constants(samples.ids().into_iter(), 0.0)),
            Function::Constant(c) => Ok(Sampled::constants(
                samples.ids().into_iter(),
                c.into_inner(),
            )),
            Function::Linear(f) => ensure_finite_sampled_results(
                "polynomial function",
                f.evaluate_samples(samples, atol)?,
            ),
            Function::Quadratic(f) => ensure_finite_sampled_results(
                "polynomial function",
                f.evaluate_samples(samples, atol)?,
            ),
            Function::Polynomial(f) => ensure_finite_sampled_results(
                "polynomial function",
                f.evaluate_samples(samples, atol)?,
            ),
            Function::Expression(_) => samples.try_map_ref(|state| self.evaluate(state, atol)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::random::*;
    use crate::{coeff, linear, monomial, quadratic};
    use ::approx::AbsDiffEq;
    use proptest::prelude::*;

    fn function_and_samples() -> impl Strategy<Value = (Function, Sampled<crate::v1::State>)> {
        Function::arbitrary()
            .prop_flat_map(|f| {
                let ids = f.required_ids();
                let state = arbitrary_state(ids);
                let samples = arbitrary_samples(SamplesParameters::default(), state);
                (Just(f), samples)
            })
            .boxed()
    }

    fn function_and_state() -> impl Strategy<Value = (Function, crate::v1::State)> {
        Function::arbitrary()
            .prop_flat_map(|function| {
                let state = arbitrary_state(function.required_ids());
                (Just(function), state)
            })
            .boxed()
    }

    fn functions_and_state() -> impl Strategy<Value = (Function, Function, crate::v1::State)> {
        (any::<Function>(), any::<Function>())
            .prop_flat_map(|(lhs, rhs)| {
                let mut ids = lhs.required_ids();
                ids.extend(rhs.required_ids());
                (Just(lhs), Just(rhs), arbitrary_state(ids))
            })
            .boxed()
    }

    proptest! {
        #[test]
        fn test_evaluate_samples((f, samples) in function_and_samples()) {
            let atol = crate::ATol::default();
            match f.evaluate_samples(&samples, atol) {
                Ok(evaluated) => {
                    for (sample_id, state) in samples.iter() {
                        let expected = f.evaluate(state, atol).unwrap();
                        let actual = *evaluated.get(*sample_id).unwrap();
                        prop_assert!(
                            actual.abs_diff_eq(&expected, 1e-9),
                            "sample_id = {sample_id:?}, expected = {expected}, actual = {actual}"
                        );
                    }
                }
                Err(_) => {
                    prop_assert!(
                        samples
                            .iter()
                            .any(|(_, state)| f.evaluate(state, atol).is_err()),
                        "sample evaluation failed although every scalar evaluation succeeded",
                    );
                }
            }
        }

        #[test]
        fn partial_evaluation_does_not_capture_atol(
            (function, state) in function_and_state(),
        ) {
            let coarse_atol = crate::ATol::new(1e-3).unwrap();
            let fine_atol = crate::ATol::new(1e-12).unwrap();
            let mut coarse_partial = function.clone();
            let mut fine_partial = function.clone();
            let coarse_result = coarse_partial.partial_evaluate(&state, coarse_atol);
            let fine_result = fine_partial.partial_evaluate(&state, fine_atol);

            match (coarse_result, fine_result) {
                (Ok(()), Ok(())) => {
                    prop_assert_eq!(&coarse_partial, &fine_partial);
                    for final_atol in [coarse_atol, fine_atol] {
                        let direct = function.evaluate(&state, final_atol);
                        let staged = coarse_partial
                            .evaluate(&crate::v1::State::default(), final_atol);
                        match (direct, staged) {
                            (Ok(expected), Ok(actual)) => {
                                let scale = 1.0_f64.max(expected.abs());
                                prop_assert!(
                                    (actual - expected).abs() <= 1e-9 * scale,
                                    "expected {expected}, actual {actual}, function {function:?}",
                                );
                            }
                            (Err(expected), Err(actual)) => {
                                prop_assert_eq!(expected.to_string(), actual.to_string());
                            }
                            (expected, actual) => {
                                prop_assert!(
                                    false,
                                    "direct and staged evaluation disagree: direct={expected:?}, staged={actual:?}, function={function:?}",
                                );
                            }
                        }
                    }
                }
                (Err(coarse), Err(fine)) => {
                    prop_assert_eq!(coarse.to_string(), fine.to_string());
                }
                (coarse, fine) => {
                    prop_assert!(
                        false,
                        "partial evaluation depends on atol: coarse={coarse:?}, fine={fine:?}, function={function:?}",
                    );
                }
            }
        }

        #[test]
        fn addition_evaluation_matches_operand_sum(
            (lhs, rhs, state) in functions_and_state(),
        ) {
            let atol = crate::ATol::default();
            let lhs_value = lhs.evaluate(&state, atol);
            let rhs_value = rhs.evaluate(&state, atol);
            let sum = (lhs + rhs).expect("generated finite coefficients can be added");
            let actual = sum.evaluate(&state, atol);

            match (lhs_value, rhs_value) {
                (Ok(lhs), Ok(rhs)) if (lhs + rhs).is_finite() => {
                    let expected = lhs + rhs;
                    let actual = actual.expect("defined operands have a defined sum");
                    let scale = 1.0_f64.max(lhs.abs() + rhs.abs());
                    prop_assert!(
                        (actual - expected).abs() <= 1e-9 * scale,
                        "expected {expected}, actual {actual}, lhs {lhs}, rhs {rhs}",
                    );
                }
                _ => {
                    prop_assert!(
                        actual.is_err(),
                        "a sum must preserve operand domain and finite-result failures",
                    );
                }
            }
        }
    }

    #[test]
    fn polynomial_evaluation_rejects_non_finite_results() {
        let function = Function::from((coeff!(f64::MAX) * linear!(1)).unwrap());

        for value in [2.0, f64::NAN] {
            let state = crate::v1::State::from_iter([(1, value)]);
            let error = function
                .evaluate(&state, crate::ATol::default())
                .unwrap_err();
            assert!(matches!(
                error.downcast_ref::<FunctionEvaluationError>(),
                Some(FunctionEvaluationError::NonFiniteResult { .. })
            ));
        }
    }

    #[test]
    fn polynomial_sample_evaluation_matches_scalar_finite_checks() {
        let function = Function::from((coeff!(f64::MAX) * linear!(1)).unwrap());
        let samples = Sampled::from(crate::v1::State::from_iter([(1, 2.0)]));

        let error = function
            .evaluate_samples(&samples, crate::ATol::default())
            .unwrap_err();
        assert!(matches!(
            error.downcast_ref::<FunctionEvaluationError>(),
            Some(FunctionEvaluationError::NonFiniteResult { .. })
        ));
    }

    #[test]
    fn compact_partial_evaluation_preserves_coefficient_error_and_is_atomic() {
        let functions = [
            Function::Linear(crate::Linear::single_term(linear!(1), coeff!(f64::MAX))),
            Function::Quadratic(crate::Quadratic::single_term(
                quadratic!(1, 2),
                coeff!(f64::MAX),
            )),
            Function::Polynomial(crate::Polynomial::single_term(
                monomial!(1, 2, 3),
                coeff!(f64::MAX),
            )),
        ];
        let state = crate::v1::State::from_iter([(1, f64::MAX)]);

        for mut function in functions {
            let original = function.clone();
            let error = function
                .partial_evaluate(&state, crate::ATol::default())
                .unwrap_err();

            assert!(error.is::<crate::CoefficientError>());
            assert_eq!(function, original);
        }
    }

    #[test]
    fn compact_partial_evaluation_normalizes_after_success() {
        let state = crate::v1::State::from_iter([(1, 2.0)]);

        let mut linear = Function::Linear(crate::Linear::single_term(linear!(1), coeff!(3.0)));
        linear
            .partial_evaluate(&state, crate::ATol::default())
            .unwrap();
        assert_eq!(linear, Function::try_from(6.0).unwrap());

        let mut quadratic =
            Function::Quadratic(crate::Quadratic::single_term(quadratic!(1, 2), coeff!(3.0)));
        quadratic
            .partial_evaluate(&state, crate::ATol::default())
            .unwrap();
        assert!(matches!(quadratic, Function::Linear(_)));

        let mut polynomial = Function::Polynomial(crate::Polynomial::single_term(
            monomial!(1, 2, 3),
            coeff!(3.0),
        ));
        polynomial
            .partial_evaluate(&state, crate::ATol::default())
            .unwrap();
        assert!(matches!(polynomial, Function::Quadratic(_)));
    }

    #[test]
    fn borrowed_replacement_preserves_disjoint_compact_canonicalization() {
        let canonical = Function::Polynomial(crate::Polynomial::single_term(
            monomial!(1, 2, 3),
            coeff!(1.0),
        ));
        assert!(canonical
            .partial_evaluate_replacement(&crate::v1::State::default(), crate::ATol::default())
            .unwrap()
            .is_none());

        let noncanonical =
            Function::Polynomial(crate::Polynomial::from(crate::Linear::from(linear!(1))));
        let replacement = noncanonical
            .partial_evaluate_replacement(&crate::v1::State::default(), crate::ATol::default())
            .unwrap();
        assert!(matches!(replacement, Some(Function::Linear(_))));

        let cases = [
            (Function::Linear(crate::Linear::zero()), Function::Zero),
            (
                Function::Linear(crate::Linear::single_term(
                    crate::LinearMonomial::Constant,
                    coeff!(2.0),
                )),
                Function::try_from(2.0).unwrap(),
            ),
            (
                Function::Quadratic(crate::Quadratic::from(crate::Linear::from(linear!(1)))),
                Function::from(linear!(1)),
            ),
            (
                Function::Polynomial(crate::Polynomial::from(crate::Quadratic::from(quadratic!(
                    1, 2
                )))),
                Function::from(quadratic!(1, 2)),
            ),
        ];
        for (noncanonical, expected) in cases {
            assert_eq!(
                noncanonical
                    .partial_evaluate_replacement(
                        &crate::v1::State::default(),
                        crate::ATol::default(),
                    )
                    .unwrap(),
                Some(expected),
            );
        }
    }

    #[test]
    fn expression_partial_evaluation_skips_disjoint_state_without_rebuilding() {
        let mut function = Function::from(linear!(2)).abs();
        let original = function.clone();
        let (instructions_pointer, instructions_len) = match &function {
            Function::Expression(expression) => {
                let instructions = instructions(expression);
                (instructions.as_ptr(), instructions.len())
            }
            _ => panic!("absolute value of a variable must be an expression"),
        };

        function
            .partial_evaluate(
                &crate::v1::State::from_iter([(1, 3.0)]),
                crate::ATol::default(),
            )
            .unwrap();

        assert_eq!(function, original);
        let Function::Expression(expression) = &function else {
            panic!("a disjoint state must preserve the expression variant");
        };
        assert_eq!(instructions(expression).len(), instructions_len);
        assert_eq!(instructions(expression).as_ptr(), instructions_pointer);
    }

    #[test]
    fn expression_partial_evaluation_normalizes_disjoint_compact_atoms() {
        let mut function =
            Function::Polynomial(crate::Polynomial::from(crate::Linear::from(linear!(1)))).abs();

        function
            .partial_evaluate(&crate::v1::State::default(), crate::ATol::default())
            .unwrap();

        assert_eq!(function, Function::from(linear!(1)).abs());
    }

    #[test]
    fn expression_partial_evaluation_folds_disjoint_closed_subexpressions() {
        let mut function = crate::function::operation::associative_expression(
            AssociativeOperator::Add,
            Function::try_from(1.0).unwrap(),
            Function::try_from(2.0).unwrap(),
        );

        function
            .partial_evaluate(&crate::v1::State::default(), crate::ATol::default())
            .unwrap();

        assert_eq!(function, Function::try_from(3.0).unwrap());
    }

    #[test]
    fn expression_partial_evaluation_reports_disjoint_closed_overflow_atomically() {
        let mut function = crate::function::operation::associative_expression(
            AssociativeOperator::Mul,
            Function::try_from(f64::MAX).unwrap(),
            Function::try_from(2.0).unwrap(),
        );
        let original = function.clone();

        let error = function
            .partial_evaluate(&crate::v1::State::default(), crate::ATol::default())
            .unwrap_err();

        assert!(matches!(
            error.downcast_ref::<FunctionEvaluationError>(),
            Some(FunctionEvaluationError::NonFiniteResult { .. })
        ));
        assert_eq!(function, original);
    }

    #[test]
    fn composite_evaluation_reports_all_missing_ids() {
        let function = Function::from(linear!(1))
            .abs()
            .min(Function::from(linear!(2)).abs());

        let error = function
            .evaluate(&crate::v1::State::default(), crate::ATol::default())
            .unwrap_err();
        let missing = error
            .downcast_ref::<crate::MissingStateEntries>()
            .expect("missing state entries must remain a typed signal");
        assert_eq!(missing.ids, crate::variable_ids!(1, 2));
    }

    #[test]
    fn composite_evaluation_reports_missing_ids_before_domain_errors() {
        let undefined = (Function::from(linear!(1)) / Function::zero())
            .expect("division by a Function is representable");
        let function = (undefined + Function::from(linear!(2)))
            .expect("addition of composed Functions is representable");

        let error = function
            .evaluate(
                &crate::v1::State::from_iter([(1, 1.0)]),
                crate::ATol::default(),
            )
            .unwrap_err();
        let missing = error
            .downcast_ref::<crate::MissingStateEntries>()
            .expect("expression shape validation precedes domain evaluation");
        assert_eq!(missing.ids, crate::variable_ids!(2));
    }

    #[test]
    fn deep_expression_evaluation_and_partial_evaluation_are_iterative() {
        let mut function = Function::from(linear!(1));
        for _ in 0..4096 {
            function = function.powi(2);
        }

        let state = crate::v1::State::from_iter([(1, 1.0)]);
        assert_eq!(
            function.evaluate(&state, crate::ATol::default()).unwrap(),
            1.0
        );

        function
            .partial_evaluate(&state, crate::ATol::default())
            .unwrap();
        assert_eq!(as_constant(&function), Some(1.0));
    }

    #[test]
    fn partial_evaluation_defers_a_closed_division_domain_check_to_final_evaluation() {
        let coarse_atol = crate::ATol::new(1e-6).unwrap();
        let fine_atol = crate::ATol::new(1e-9).unwrap();
        let tiny = 1e-8;
        let reciprocal =
            (Function::one() / Function::from(linear!(1))).expect("division is representable");
        let mut function =
            (reciprocal + Function::from(linear!(2))).expect("addition is representable");

        function
            .partial_evaluate(&crate::v1::State::from_iter([(1, tiny)]), coarse_atol)
            .unwrap();
        assert!(matches!(function, Function::Expression(_)));
        assert_eq!(
            function.required_ids(),
            VariableIDSet::from([VariableID::from(2)])
        );

        let remaining_state = crate::v1::State::from_iter([(2, 0.0)]);
        let error = function
            .evaluate(&remaining_state, coarse_atol)
            .unwrap_err();
        assert!(matches!(
            error.downcast_ref::<FunctionEvaluationError>(),
            Some(FunctionEvaluationError::DivisionByZero)
        ));
        assert_eq!(
            function.evaluate(&remaining_state, fine_atol).unwrap(),
            1.0 / tiny
        );
    }

    #[test]
    fn partial_evaluation_defers_zero_sensitive_unary_checks_to_final_atol() {
        let coarse_atol = crate::ATol::new(1e-6).unwrap();
        let fine_atol = crate::ATol::new(1e-9).unwrap();
        let tiny = 1e-8;
        let assigned = crate::v1::State::from_iter([(1, tiny)]);
        let empty = crate::v1::State::default();

        let mut signum = Function::from(linear!(1)).signum();
        signum.partial_evaluate(&assigned, coarse_atol).unwrap();
        assert!(matches!(signum, Function::Expression(_)));
        assert_eq!(signum.evaluate(&empty, coarse_atol).unwrap(), 0.0);
        assert_eq!(signum.evaluate(&empty, fine_atol).unwrap(), 1.0);

        let mut inverse = Function::from(linear!(1)).powi(-1);
        inverse.partial_evaluate(&assigned, coarse_atol).unwrap();
        assert!(matches!(inverse, Function::Expression(_)));
        let error = inverse.evaluate(&empty, coarse_atol).unwrap_err();
        assert!(matches!(
            error.downcast_ref::<FunctionEvaluationError>(),
            Some(FunctionEvaluationError::ZeroToNegativeIntegerPower { exponent: -1 })
        ));
        assert_eq!(inverse.evaluate(&empty, fine_atol).unwrap(), 1.0 / tiny);
    }

    #[test]
    fn zero_sensitive_operations_include_the_atol_boundary() {
        let atol = crate::ATol::new(1e-6).unwrap();
        let x = Function::from(linear!(1));
        let signum = x.clone().signum();
        let reciprocal = (Function::one() / x.clone()).unwrap();
        let inverse = x.powi(-1);

        for value in [*atol, -*atol] {
            let state = crate::v1::State::from_iter([(1, value)]);
            assert_eq!(signum.evaluate(&state, atol).unwrap(), 0.0);

            let division_error = reciprocal.evaluate(&state, atol).unwrap_err();
            assert!(matches!(
                division_error.downcast_ref::<FunctionEvaluationError>(),
                Some(FunctionEvaluationError::DivisionByZero)
            ));

            let power_error = inverse.evaluate(&state, atol).unwrap_err();
            assert!(matches!(
                power_error.downcast_ref::<FunctionEvaluationError>(),
                Some(FunctionEvaluationError::ZeroToNegativeIntegerPower { exponent: -1 })
            ));
        }

        for (value, expected_sign) in [(2.0 * *atol, 1.0), (-2.0 * *atol, -1.0)] {
            let state = crate::v1::State::from_iter([(1, value)]);
            assert_eq!(signum.evaluate(&state, atol).unwrap(), expected_sign);
            assert_eq!(reciprocal.evaluate(&state, atol).unwrap(), 1.0 / value);
            assert_eq!(inverse.evaluate(&state, atol).unwrap(), 1.0 / value);
        }
    }

    #[test]
    fn sample_evaluation_uses_the_caller_atol_for_zero_sensitive_operations() {
        let coarse_atol = crate::ATol::new(1e-6).unwrap();
        let fine_atol = crate::ATol::new(1e-9).unwrap();
        let tiny = 1e-8;
        let samples = Sampled::from(crate::v1::State::from_iter([(1, tiny)]));
        let sample_id = crate::SampleID::from(0);
        let x = Function::from(linear!(1));

        let signum = x.clone().signum();
        assert_eq!(
            *signum
                .evaluate_samples(&samples, coarse_atol)
                .unwrap()
                .get(sample_id)
                .unwrap(),
            0.0
        );
        assert_eq!(
            *signum
                .evaluate_samples(&samples, fine_atol)
                .unwrap()
                .get(sample_id)
                .unwrap(),
            1.0
        );

        let reciprocal = (Function::one() / x.clone()).unwrap();
        let division_error = reciprocal
            .evaluate_samples(&samples, coarse_atol)
            .unwrap_err();
        assert!(matches!(
            division_error.downcast_ref::<FunctionEvaluationError>(),
            Some(FunctionEvaluationError::DivisionByZero)
        ));
        assert_eq!(
            *reciprocal
                .evaluate_samples(&samples, fine_atol)
                .unwrap()
                .get(sample_id)
                .unwrap(),
            1.0 / tiny
        );

        let inverse = x.powi(-1);
        let power_error = inverse.evaluate_samples(&samples, coarse_atol).unwrap_err();
        assert!(matches!(
            power_error.downcast_ref::<FunctionEvaluationError>(),
            Some(FunctionEvaluationError::ZeroToNegativeIntegerPower { exponent: -1 })
        ));
        assert_eq!(
            *inverse
                .evaluate_samples(&samples, fine_atol)
                .unwrap()
                .get(sample_id)
                .unwrap(),
            1.0 / tiny
        );
    }

    #[test]
    fn partial_evaluation_reports_an_earlier_operation_error_before_later_push_error() {
        let overflowing_operation = super::operation::associative_expression(
            AssociativeOperator::Mul,
            Function::from(coeff!(f64::MAX)),
            Function::from(linear!(1)),
        );
        let later_overflowing_push = Function::from((coeff!(f64::MAX) * linear!(2)).unwrap());
        let mut function = super::operation::associative_expression(
            AssociativeOperator::Add,
            overflowing_operation,
            later_overflowing_push,
        );
        let original = function.clone();

        let error = function
            .partial_evaluate(
                &crate::v1::State::from_iter([(1, 2.0), (2, 2.0)]),
                crate::ATol::default(),
            )
            .unwrap_err();
        assert!(matches!(
            error.downcast_ref::<FunctionEvaluationError>(),
            Some(FunctionEvaluationError::NonFiniteResult { .. })
        ));
        assert_eq!(function, original);
    }

    #[test]
    fn associative_instructions_preserve_grouping() {
        let large = Function::try_from(1e16).unwrap();
        let negative_large = Function::try_from(-1e16).unwrap();
        let one = Function::one();

        let left_grouped = super::operation::associative_expression(
            AssociativeOperator::Add,
            super::operation::associative_expression(
                AssociativeOperator::Add,
                large.clone(),
                negative_large.clone(),
            ),
            one.clone(),
        );
        let right_grouped = super::operation::associative_expression(
            AssociativeOperator::Add,
            large,
            super::operation::associative_expression(AssociativeOperator::Add, negative_large, one),
        );

        let state = crate::v1::State::default();
        assert_eq!(
            left_grouped
                .evaluate(&state, crate::ATol::default())
                .unwrap(),
            1.0
        );
        assert_eq!(
            right_grouped
                .evaluate(&state, crate::ATol::default())
                .unwrap(),
            0.0
        );
    }
}
