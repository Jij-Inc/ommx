use super::Function;
use crate::Coefficient;

/// A recoverable failure caused by evaluating a [`Function`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FunctionEvaluationError {
    #[error("division by zero while evaluating Function")]
    DivisionByZero,
    #[error("{operation} received a non-finite operand: {value}")]
    NonFiniteOperand { operation: &'static str, value: f64 },
    #[error("{operation} produced a non-finite value")]
    NonFiniteResult { operation: &'static str },
    #[error("real power is undefined for base {base} and exponent {exponent}")]
    PowerOutsideRealDomain { base: f64, exponent: f64 },
}

/// Operator for a function with one operand.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    crate::logical_memory::LogicalMemoryProfile,
)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum UnaryOperator {
    Neg,
    Abs,
    Signum,
}

/// A validated unary operation.
#[derive(Debug, Clone, PartialEq, crate::logical_memory::LogicalMemoryProfile)]
pub struct UnaryOperation {
    operator: UnaryOperator,
    operand: Box<Function>,
}

impl UnaryOperation {
    pub fn operator(&self) -> UnaryOperator {
        self.operator
    }

    pub fn operand(&self) -> &Function {
        &self.operand
    }

    pub fn into_parts(self) -> (UnaryOperator, Function) {
        (self.operator, *self.operand)
    }
}

/// Operator for an ordered function with two or more operands.
///
/// Operands are evaluated from left to right. A nested n-ary operation preserves
/// grouping unless it appears as the first operand of the same operation, where
/// flattening preserves the left fold.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    crate::logical_memory::LogicalMemoryProfile,
)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum NaryOperator {
    Add,
    Mul,
    Min,
    Max,
}

/// A validated ordered n-ary operation containing at least two operands.
#[derive(Debug, Clone, PartialEq, crate::logical_memory::LogicalMemoryProfile)]
pub struct NaryOperation {
    operator: NaryOperator,
    operands: Vec<Function>,
}

impl NaryOperation {
    pub fn operator(&self) -> NaryOperator {
        self.operator
    }

    pub fn operands(&self) -> &[Function] {
        &self.operands
    }

    pub fn into_parts(self) -> (NaryOperator, Vec<Function>) {
        (self.operator, self.operands)
    }
}

/// Operator for an ordered function with a left- and right-hand operand.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    crate::logical_memory::LogicalMemoryProfile,
)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum BinaryOperator {
    Div,
    Pow,
}

/// A validated ordered binary operation.
#[derive(Debug, Clone, PartialEq, crate::logical_memory::LogicalMemoryProfile)]
pub struct BinaryOperation {
    operator: BinaryOperator,
    lhs: Box<Function>,
    rhs: Box<Function>,
}

impl BinaryOperation {
    pub fn operator(&self) -> BinaryOperator {
        self.operator
    }

    pub fn lhs(&self) -> &Function {
        &self.lhs
    }

    pub fn rhs(&self) -> &Function {
        &self.rhs
    }

    pub fn into_parts(self) -> (BinaryOperator, Function, Function) {
        (self.operator, *self.lhs, *self.rhs)
    }
}

impl Function {
    /// Construct the exact validated expression shape used at deserialization
    /// boundaries. In particular, do not eagerly expand polynomial products
    /// supplied by an untrusted wire payload.
    pub(crate) fn nary_expression(operator: NaryOperator, operands: Vec<Function>) -> Self {
        debug_assert!(operands.len() >= 2);
        Function::Nary(NaryOperation { operator, operands })
    }

    /// Construct an ordered binary expression without algebraic folding.
    /// Deserializers use this to preserve wire evaluation order and domains.
    pub(crate) fn binary_expression(
        operator: BinaryOperator,
        lhs: Function,
        rhs: Function,
    ) -> Self {
        Function::Binary(BinaryOperation {
            operator,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        })
    }

    pub(crate) fn unary_operation(operator: UnaryOperator, operand: Function) -> Self {
        if let Function::Unary(inner) = operand {
            if operator == UnaryOperator::Neg && inner.operator == UnaryOperator::Neg {
                return *inner.operand;
            }
            if operator == inner.operator
                && matches!(operator, UnaryOperator::Abs | UnaryOperator::Signum)
            {
                return Function::Unary(inner);
            }
            return Function::Unary(UnaryOperation {
                operator,
                operand: Box::new(Function::Unary(inner)),
            });
        }

        if let Some(value) = operand.as_constant() {
            let value = match operator {
                UnaryOperator::Neg => -value,
                UnaryOperator::Abs => value.abs(),
                UnaryOperator::Signum if value < 0.0 => -1.0,
                UnaryOperator::Signum if value > 0.0 => 1.0,
                UnaryOperator::Signum => 0.0,
            };
            return Function::try_from(value).expect("operations on finite constants stay finite");
        }

        if operator == UnaryOperator::Neg && operand.is_polynomial() {
            return -operand;
        }

        Function::Unary(UnaryOperation {
            operator,
            operand: Box::new(operand),
        })
    }

    pub(crate) fn nary_operation(operator: NaryOperator, operands: Vec<Function>) -> Self {
        debug_assert!(operands.len() >= 2);
        let mut operands = operands.into_iter();
        let first = operands
            .next()
            .expect("n-ary operation always has at least two operands");
        let (mut flattened, extended_explicit_nary) = match first {
            Function::Nary(inner) if inner.operator == operator => (inner.operands, true),
            first => (vec![first], false),
        };
        flattened.extend(operands);

        if flattened.len() == 1 {
            return flattened.pop().expect("length checked");
        }

        // Once an explicit expression node exists (notably after wire or serde
        // parsing), appending another operand must not turn it back into a
        // compact polynomial. Doing so would change its represented
        // left-to-right floating-point evaluation and can expand a small
        // product-of-sums payload into exponentially many terms.
        if extended_explicit_nary {
            return Function::nary_expression(operator, flattened);
        }

        if matches!(operator, NaryOperator::Min | NaryOperator::Max)
            && flattened
                .iter()
                .all(|operand| operand.as_constant().is_some())
        {
            let mut values = flattened.iter().map(|operand| {
                operand
                    .as_constant()
                    .expect("all operands were checked as constants")
            });
            let first = values.next().expect("n-ary operation is non-empty");
            let value = values.fold(first, |acc, value| match operator {
                NaryOperator::Min => acc.min(value),
                NaryOperator::Max => acc.max(value),
                _ => unreachable!(),
            });
            return Function::try_from(value).expect("minimum/maximum of finite values is finite");
        }

        if matches!(operator, NaryOperator::Add | NaryOperator::Mul)
            && flattened.iter().all(Function::is_polynomial)
        {
            let mut operands = flattened.clone().into_iter();
            let first = operands
                .next()
                .expect("n-ary operation always has at least two operands");
            let folded = operands.try_fold(first, |acc, operand| match operator {
                NaryOperator::Add => acc + operand,
                NaryOperator::Mul => acc * operand,
                _ => unreachable!(),
            });
            if let Ok(folded) = folded {
                return folded;
            }
        }
        Function::nary_expression(operator, flattened)
    }

    pub(crate) fn binary_operation(operator: BinaryOperator, lhs: Function, rhs: Function) -> Self {
        if operator == BinaryOperator::Div && lhs.is_polynomial() {
            if let Some(rhs_value) = rhs.as_constant() {
                if let Ok(rhs_coefficient) = Coefficient::try_from(rhs_value) {
                    if let Ok(divided) = lhs.clone() / rhs_coefficient {
                        return divided;
                    }
                }
            }
        }
        if let (Some(lhs_value), Some(rhs_value)) = (lhs.as_constant(), rhs.as_constant()) {
            let value = match operator {
                BinaryOperator::Div if rhs_value != 0.0 => Some(lhs_value / rhs_value),
                BinaryOperator::Div => None,
                BinaryOperator::Pow => Some(lhs_value.powf(rhs_value)),
            };
            if let Some(value) = value.filter(|value| value.is_finite()) {
                return Function::try_from(value).expect("value was checked as finite");
            }
        }
        Function::binary_expression(operator, lhs, rhs)
    }

    /// Absolute value of this function.
    pub fn abs(self) -> Self {
        Self::unary_operation(UnaryOperator::Abs, self)
    }

    /// Mathematical sign function, with `signum(0) = 0`.
    pub fn signum(self) -> Self {
        Self::unary_operation(UnaryOperator::Signum, self)
    }

    /// Pointwise minimum of two functions.
    pub fn min(self, rhs: Self) -> Self {
        Self::nary_operation(NaryOperator::Min, vec![self, rhs])
    }

    /// Pointwise maximum of two functions.
    pub fn max(self, rhs: Self) -> Self {
        Self::nary_operation(NaryOperator::Max, vec![self, rhs])
    }

    /// Raise this function to a function-valued exponent using real-valued semantics.
    pub fn pow(self, exponent: Self) -> Self {
        Self::binary_operation(BinaryOperator::Pow, self, exponent)
    }

    pub(crate) fn as_constant(&self) -> Option<f64> {
        match self {
            Function::Zero => Some(0.0),
            Function::Constant(value) => Some(value.into_inner()),
            Function::Linear(value) if value.degree().into_inner() == 0 => {
                Some(value.constant_term())
            }
            Function::Quadratic(value) if value.degree().into_inner() == 0 => {
                Some(value.constant_term())
            }
            Function::Polynomial(value) if value.degree().into_inner() == 0 => {
                Some(value.constant_term())
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, linear, ATol, Evaluate, Substitute};
    use std::collections::HashMap;

    fn constant(value: f64) -> Function {
        Function::try_from(value).unwrap()
    }

    fn state(value: f64) -> crate::v1::State {
        crate::v1::State::from(HashMap::from([(1, value)]))
    }

    #[test]
    fn unary_operations_use_mathematical_signum() {
        let x = Function::from(linear!(1));
        assert_eq!(
            x.clone()
                .abs()
                .evaluate(&state(-2.5), ATol::default())
                .unwrap(),
            2.5
        );
        let signum = x.signum();
        assert_eq!(signum.evaluate(&state(-0.0), ATol::default()).unwrap(), 0.0);
        assert_eq!(signum.evaluate(&state(0.0), ATol::default()).unwrap(), 0.0);
        assert_eq!(
            signum.evaluate(&state(-3.0), ATol::default()).unwrap(),
            -1.0
        );
        assert_eq!(signum.evaluate(&state(3.0), ATol::default()).unwrap(), 1.0);
    }

    #[test]
    fn min_max_flatten_and_preserve_operand_order() {
        let x = Function::from(linear!(1));
        let expression = x.clone().min(constant(2.0)).min(constant(-1.0));
        let Function::Nary(operation) = &expression else {
            panic!("minimum should remain an n-ary expression");
        };
        assert_eq!(operation.operator(), NaryOperator::Min);
        assert_eq!(operation.operands().len(), 3);
        assert_eq!(
            expression.evaluate(&state(4.0), ATol::default()).unwrap(),
            -1.0
        );

        let maximum = x.max(constant(2.0));
        assert_eq!(
            maximum.evaluate(&state(-3.0), ATol::default()).unwrap(),
            2.0
        );
    }

    #[test]
    fn nary_operations_preserve_right_grouping() {
        let a = Function::from(linear!(1)).abs();
        let b = Function::from(linear!(2)).abs();
        let c = Function::from(linear!(3)).abs();

        let left_grouped = ((a.clone() + b.clone()).unwrap() + c.clone()).unwrap();
        let right_grouped = (a + (b + c).unwrap()).unwrap();

        let Function::Nary(left) = &left_grouped else {
            panic!("composite addition should use an n-ary operation");
        };
        assert_eq!(left.operands().len(), 3);

        let Function::Nary(right) = &right_grouped else {
            panic!("composite addition should use an n-ary operation");
        };
        assert_eq!(right.operands().len(), 2);
        assert!(matches!(right.operands()[1], Function::Nary(_)));
        assert!(left_grouped != right_grouped);
    }

    #[test]
    fn coefficient_lhs_preserves_order_for_composite_arithmetic() {
        let composite = Function::from(linear!(1)).abs();
        let expressions = [
            (coeff!(2.0) + composite.clone()).unwrap(),
            (coeff!(2.0) * composite.clone()).unwrap(),
            (coeff!(2.0) - composite).unwrap(),
        ];

        for expression in expressions {
            let Function::Nary(operation) = expression else {
                panic!("coefficient/composite arithmetic should use an n-ary operation");
            };
            assert_eq!(operation.operands()[0].as_constant(), Some(2.0));
        }
    }

    #[test]
    fn arithmetic_is_closed_over_composite_functions() {
        let x = Function::from(linear!(1));
        let absolute = x.clone().abs();
        let sum = (absolute.clone() + constant(1.0)).unwrap();
        let product = (x.clone() * absolute.clone()).unwrap();
        let negated = -absolute;

        assert!(matches!(sum, Function::Nary(_)));
        assert!(matches!(product, Function::Nary(_)));
        assert!(matches!(negated, Function::Unary(_)));
        assert_eq!(sum.evaluate(&state(-2.0), ATol::default()).unwrap(), 3.0);
        assert_eq!(
            product.evaluate(&state(-2.0), ATol::default()).unwrap(),
            -4.0
        );
        assert_eq!(
            negated.evaluate(&state(-2.0), ATol::default()).unwrap(),
            -2.0
        );
    }

    #[test]
    fn division_and_power_report_undefined_real_values() {
        let x = Function::from(linear!(1));
        let reciprocal = (Function::one() / x.clone()).unwrap();
        assert_eq!(
            reciprocal.evaluate(&state(2.0), ATol::default()).unwrap(),
            0.5
        );
        let error = reciprocal
            .evaluate(&state(0.0), ATol::default())
            .unwrap_err();
        assert!(matches!(
            error.downcast_ref::<FunctionEvaluationError>(),
            Some(FunctionEvaluationError::DivisionByZero)
        ));

        let square_root = x.pow(constant(0.5));
        assert_eq!(
            square_root.evaluate(&state(4.0), ATol::default()).unwrap(),
            2.0
        );
        let error = square_root
            .evaluate(&state(-1.0), ATol::default())
            .unwrap_err();
        assert!(matches!(
            error.downcast_ref::<FunctionEvaluationError>(),
            Some(FunctionEvaluationError::PowerOutsideRealDomain { .. })
        ));
        assert_eq!(
            constant(0.0)
                .pow(constant(0.0))
                .evaluate(&crate::v1::State::default(), ATol::default())
                .unwrap(),
            1.0
        );
    }

    #[test]
    fn division_by_a_nonzero_constant_keeps_the_polynomial_fast_path() {
        let x = Function::from(linear!(1));
        let divided = Function::binary_operation(BinaryOperator::Div, x, constant(2.0));
        assert!(divided.is_polynomial());
        assert_eq!(divided.evaluate(&state(4.0), ATol::default()).unwrap(), 2.0);
    }

    #[test]
    fn simplification_does_not_remove_a_partial_domain() {
        let x = Function::from(linear!(1));
        let reciprocal = (Function::one() / x).unwrap();
        let expression = (Function::zero() * reciprocal).unwrap();
        assert!(matches!(expression, Function::Nary(_)));
        assert!(expression.evaluate(&state(0.0), ATol::default()).is_err());
    }

    #[test]
    fn composite_partial_evaluation_and_substitution_recurse() {
        let x = Function::from(linear!(1));
        let mut expression = (x.clone().abs() + constant(1.0)).unwrap();
        expression
            .partial_evaluate(&state(-2.0), ATol::default())
            .unwrap();
        assert_eq!(expression.as_constant(), Some(3.0));

        let substituted = x
            .signum()
            .substitute_one(1.into(), &constant(-4.0))
            .unwrap();
        assert_eq!(substituted.as_constant(), Some(-1.0));
    }

    #[test]
    fn rewrites_preserve_ordered_expression_evaluation() {
        let y = Function::from(linear!(2));
        let scaled_x = Function::from((coeff!(1e150) * linear!(1)).unwrap());
        let expression = ((y.abs() * scaled_x.clone()).unwrap() * scaled_x).unwrap();
        let state = crate::v1::State::from(HashMap::from([(1, 1e-200), (2, 1.0)]));
        let expected = expression.evaluate(&state, ATol::default()).unwrap();
        assert!(expected > 0.0);

        let mut partially_evaluated = expression.clone();
        partially_evaluated
            .partial_evaluate(
                &crate::v1::State::from(HashMap::from([(2, 1.0)])),
                ATol::default(),
            )
            .unwrap();
        assert_eq!(
            partially_evaluated
                .evaluate(
                    &crate::v1::State::from(HashMap::from([(1, 1e-200)])),
                    ATol::default(),
                )
                .unwrap()
                .to_bits(),
            expected.to_bits(),
        );

        let substituted = expression.substitute_one(2.into(), &constant(1.0)).unwrap();
        assert_eq!(
            substituted
                .evaluate(
                    &crate::v1::State::from(HashMap::from([(1, 1e-200)])),
                    ATol::default(),
                )
                .unwrap()
                .to_bits(),
            expected.to_bits(),
        );
    }

    #[test]
    fn appending_to_an_explicit_nary_never_eagerly_expands_it() {
        let scaled_x = Function::from((coeff!(1e150) * linear!(1)).unwrap());
        let explicit =
            Function::nary_expression(NaryOperator::Mul, vec![scaled_x.clone(), scaled_x]);
        let appended = (explicit * constant(0.5)).unwrap();
        let Function::Nary(operation) = &appended else {
            panic!("appending to an explicit n-ary expression must preserve the AST");
        };
        assert_eq!(operation.operands().len(), 3);
        let state = state(1e-200);
        let operand: f64 = 1e150 * 1e-200;
        assert_eq!(
            appended
                .evaluate(&state, ATol::default())
                .unwrap()
                .to_bits(),
            (operand * operand * 0.5).to_bits(),
        );

        let product_of_sums = Function::nary_expression(
            NaryOperator::Mul,
            (1..=20)
                .map(|id| {
                    (Function::from(linear!(id)) + constant(1.0))
                        .expect("polynomial addition is valid")
                })
                .collect(),
        );
        let appended = (product_of_sums * constant(2.0)).unwrap();
        let Function::Nary(operation) = appended else {
            panic!("explicit product-of-sums must not be expanded");
        };
        assert_eq!(operation.operands().len(), 21);
    }
}
