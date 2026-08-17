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
            Function::Unary(operation) => {
                let (operator, operand) = operation.into_parts();
                Function::unary_operation(operator, operand.partially_evaluated(state, atol)?)
            }
            Function::Nary(operation) => {
                let (operator, operands) = operation.into_parts();
                let operands = operands
                    .into_iter()
                    .map(|operand| operand.partially_evaluated(state, atol))
                    .collect::<crate::Result<Vec<_>>>()?;
                Function::nary_expression(operator, operands)
            }
            Function::Binary(operation) => {
                let (operator, lhs, rhs) = operation.into_parts();
                Function::binary_expression(
                    operator,
                    lhs.partially_evaluated(state, atol)?,
                    rhs.partially_evaluated(state, atol)?,
                )
            }
        };

        if result.required_ids().is_empty() && !result.is_polynomial() {
            let value = result.evaluate(&crate::v1::State::default(), atol)?;
            return Function::try_from(value).map_err(Into::into);
        }
        Ok(result)
    }
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
            Function::Unary(operation) => {
                let operand = ensure_finite(
                    "unary operation operand",
                    operation.operand().evaluate(solution, atol)?,
                )?;
                let value = match operation.operator() {
                    UnaryOperator::Neg => -operand,
                    UnaryOperator::Abs => operand.abs(),
                    UnaryOperator::Signum if operand < 0.0 => -1.0,
                    UnaryOperator::Signum if operand > 0.0 => 1.0,
                    UnaryOperator::Signum => 0.0,
                    UnaryOperator::Powi(exponent) if operand == 0.0 && exponent < 0 => {
                        return Err(FunctionEvaluationError::ZeroToNegativeIntegerPower {
                            exponent,
                        }
                        .into());
                    }
                    UnaryOperator::Powi(exponent) => operand.powi(exponent),
                };
                ensure_finite_result("unary operation", value)
            }
            Function::Nary(operation) => {
                let mut values = operation.operands().iter().map(|operand| {
                    ensure_finite("nary operation operand", operand.evaluate(solution, atol)?)
                });
                let first = values
                    .next()
                    .expect("nary operation always has at least two operands")?;
                let value = values.try_fold(first, |acc, value| {
                    let value = value?;
                    let result = match operation.operator() {
                        NaryOperator::Add => acc + value,
                        NaryOperator::Mul => acc * value,
                        NaryOperator::Min => acc.min(value),
                        NaryOperator::Max => acc.max(value),
                    };
                    ensure_finite_result("nary operation", result)
                })?;
                Ok(value)
            }
            Function::Binary(operation) => {
                let lhs = ensure_finite(
                    "binary operation left operand",
                    operation.lhs().evaluate(solution, atol)?,
                )?;
                let rhs = ensure_finite(
                    "binary operation right operand",
                    operation.rhs().evaluate(solution, atol)?,
                )?;
                let value = match operation.operator() {
                    BinaryOperator::Div if rhs == 0.0 => {
                        return Err(FunctionEvaluationError::DivisionByZero.into());
                    }
                    BinaryOperator::Div => lhs / rhs,
                };
                ensure_finite_result("binary operation", value)
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
            Function::Unary(operation) => operation.operand().required_ids(),
            Function::Nary(operation) => {
                let mut ids = VariableIDSet::default();
                for operand in operation.operands() {
                    ids.extend(operand.required_ids());
                }
                ids
            }
            Function::Binary(operation) => {
                let mut ids = operation.lhs().required_ids();
                ids.extend(operation.rhs().required_ids());
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
            | Function::Unary(_)
            | Function::Nary(_)
            | Function::Binary(_) => samples.try_map_ref(|state| self.evaluate(state, atol)),
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
}
