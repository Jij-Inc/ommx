use super::operation::{
    as_constant, from_instructions_exact, instructions, into_expression_instructions,
    into_instructions, AssociativeOperator, Atom, BinaryOperator, Instruction, UnaryOperator,
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

impl Function {
    fn partially_evaluated(
        self,
        state: &crate::v1::State,
        atol: crate::ATol,
    ) -> crate::Result<Self> {
        let result = match self {
            Function::Zero | Function::Constant(_) => self,
            Function::Linear(mut function) => {
                function.partial_evaluate(state, atol)?;
                Function::Linear(function).normalize()
            }
            Function::Quadratic(mut function) => {
                function.partial_evaluate(state, atol)?;
                Function::Quadratic(function).normalize()
            }
            Function::Polynomial(mut function) => {
                function.partial_evaluate(state, atol)?;
                Function::Polynomial(function).normalize()
            }
            Function::Expression(expression) => {
                partially_evaluate_expression(expression, state, atol)?
            }
        };

        if result.required_ids().is_empty() && !result.is_polynomial() {
            let value = result.evaluate(&crate::v1::State::default(), atol)?;
            return Function::try_from(value).map_err(Into::into);
        }
        Ok(result)
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

fn evaluate_unary(operator: UnaryOperator, operand: f64) -> crate::Result<f64> {
    let operand = ensure_finite("unary operation operand", operand)?;
    let value = match operator {
        UnaryOperator::Neg => -operand,
        UnaryOperator::Abs => operand.abs(),
        UnaryOperator::Signum if operand < 0.0 => -1.0,
        UnaryOperator::Signum if operand > 0.0 => 1.0,
        UnaryOperator::Signum => 0.0,
        UnaryOperator::Powi(exponent) if operand == 0.0 && exponent < 0 => {
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

fn evaluate_binary(operator: BinaryOperator, lhs: f64, rhs: f64) -> crate::Result<f64> {
    let lhs = ensure_finite("binary operation left operand", lhs)?;
    let rhs = ensure_finite("binary operation right operand", rhs)?;
    let value = match operator {
        BinaryOperator::Div if rhs == 0.0 => {
            return Err(FunctionEvaluationError::DivisionByZero.into());
        }
        BinaryOperator::Div => lhs / rhs,
    };
    ensure_finite_result("binary operation", value)
}

#[derive(Clone, Copy)]
struct PartialOperand {
    start: usize,
    /// A value is present exactly when this operand no longer requires state.
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
    expression: Expression,
    state: &crate::v1::State,
    atol: crate::ATol,
) -> crate::Result<Function> {
    let mut instructions = Vec::with_capacity(instructions(&expression).len());
    let mut operands = Vec::new();

    for instruction in into_expression_instructions(expression) {
        match instruction {
            Instruction::Push(atom) => {
                let evaluated = atom.into_function().partially_evaluated(state, atol)?;
                let value = as_constant(&evaluated);
                debug_assert_eq!(value.is_some(), evaluated.required_ids().is_empty());
                let start = instructions.len();
                instructions.extend(into_instructions(evaluated));
                operands.push(PartialOperand { start, value });
            }
            Instruction::Unary(operator) => {
                let operand = operands
                    .pop()
                    .expect("validated unary instruction has an operand");
                if let Some(value) = operand.value {
                    let value = evaluate_unary(operator, value)?;
                    operands.push(replace_partial_operand_with_value(
                        &mut instructions,
                        operand.start,
                        value,
                    ));
                } else {
                    instructions.push(Instruction::Unary(operator));
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
                    let value = evaluate_associative(operator, lhs_value, rhs_value)?;
                    operands.push(replace_partial_operand_with_value(
                        &mut instructions,
                        lhs.start,
                        value,
                    ));
                } else {
                    instructions.push(Instruction::Associative(operator));
                    operands.push(PartialOperand {
                        start: lhs.start,
                        value: None,
                    });
                }
            }
            Instruction::Binary(operator) => {
                let rhs = operands
                    .pop()
                    .expect("validated binary instruction has a right operand");
                let lhs = operands
                    .pop()
                    .expect("validated binary instruction has a left operand");
                if let (Some(lhs_value), Some(rhs_value)) = (lhs.value, rhs.value) {
                    let value = evaluate_binary(operator, lhs_value, rhs_value)?;
                    operands.push(replace_partial_operand_with_value(
                        &mut instructions,
                        lhs.start,
                        value,
                    ));
                } else {
                    instructions.push(Instruction::Binary(operator));
                    operands.push(PartialOperand {
                        start: lhs.start,
                        value: None,
                    });
                }
            }
        }
    }

    debug_assert_eq!(operands.len(), 1);
    Ok(from_instructions_exact(instructions)
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
        let state_ids: VariableIDSet = solution.entries.keys().map(|id| (*id).into()).collect();
        let missing_ids: VariableIDSet = self
            .required_ids()
            .difference(&state_ids)
            .copied()
            .collect();
        if !missing_ids.is_empty() {
            return Err(crate::MissingStateEntries { ids: missing_ids }.into());
        }
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
                            values.push(evaluate_unary(*operator, operand)?);
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
                            values.push(evaluate_binary(*operator, lhs, rhs)?);
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
        let evaluated = self.clone().partially_evaluated(state, atol)?;
        *self = evaluated;
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
            Function::Linear(_)
            | Function::Quadratic(_)
            | Function::Polynomial(_)
            | Function::Expression(_) => samples.try_map_ref(|state| self.evaluate(state, atol)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::random::*;
    use crate::{coeff, linear};
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

    proptest! {
        #[test]
        fn test_evaluate_samples((f, samples) in function_and_samples()) {
            let evaluated = f.evaluate_samples(&samples, crate::ATol::default()).unwrap();
            for (sample_id, state) in samples.iter() {
                let expected = f.evaluate(state, crate::ATol::default()).unwrap();
                let actual = *evaluated.get(*sample_id).unwrap();
                prop_assert!(
                    actual.abs_diff_eq(&expected, 1e-9),
                    "sample_id = {sample_id:?}, expected = {expected}, actual = {actual}"
                );
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
    fn partial_evaluation_reports_a_closed_domain_error_before_later_open_operand() {
        let reciprocal =
            (Function::one() / Function::from(linear!(1))).expect("division is representable");
        let mut function =
            (reciprocal + Function::from(linear!(2))).expect("addition is representable");

        let error = function
            .partial_evaluate(
                &crate::v1::State::from_iter([(1, 0.0)]),
                crate::ATol::default(),
            )
            .unwrap_err();
        assert!(matches!(
            error.downcast_ref::<FunctionEvaluationError>(),
            Some(FunctionEvaluationError::DivisionByZero)
        ));
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
