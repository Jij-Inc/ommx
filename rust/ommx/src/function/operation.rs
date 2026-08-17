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
    #[error("zero cannot be raised to the negative integer exponent {exponent}")]
    ZeroToNegativeIntegerPower { exponent: i32 },
}

/// Operator for a function with one operand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub(crate) enum UnaryOperator {
    Neg,
    Abs,
    Signum,
    /// Integer power with the exponent stored as an operation parameter.
    Powi(i32),
}

// This data-carrying enum is entirely inline and has no nested allocations, so
// it is a logical-memory leaf. The derive intentionally supports only
// fieldless enums; keep this manual dispatch local to the exceptional shape.
impl crate::logical_memory::LogicalMemoryProfile for UnaryOperator {
    fn visit_logical_memory<V: crate::logical_memory::LogicalMemoryVisitor>(
        &self,
        path: &mut crate::logical_memory::Path,
        visitor: &mut V,
    ) {
        visitor.visit_leaf(path, std::mem::size_of::<Self>());
    }
}

/// Operator whose two operands may be combined repeatedly.
///
/// Each instruction applies the operator to exactly two stack values. Longer
/// sums, products, minima, and maxima are represented by multiple instructions,
/// which preserves both grouping and left-to-right evaluation order.
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
pub(crate) enum AssociativeOperator {
    Add,
    Mul,
    Min,
    Max,
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
pub(crate) enum BinaryOperator {
    Div,
}

/// A non-recursive compact-polynomial operand in an expression program.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Atom {
    Zero,
    Constant(Coefficient),
    Linear(crate::Linear),
    Quadratic(crate::Quadratic),
    Polynomial(crate::Polynomial),
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
enum AtomSerde {
    Zero,
    Constant(Coefficient),
    Linear(Vec<(crate::LinearMonomial, Coefficient)>),
    Quadratic(Vec<(crate::QuadraticMonomial, Coefficient)>),
    Polynomial(Vec<(crate::MonomialDyn, Coefficient)>),
}

fn serialize_terms<M: crate::Monomial>(
    polynomial: &crate::PolynomialBase<M>,
) -> Vec<(M, Coefficient)> {
    polynomial
        .iter()
        .map(|(monomial, coefficient)| (monomial.clone(), *coefficient))
        .collect()
}

impl serde::Serialize for Atom {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let value = match self {
            Self::Zero => AtomSerde::Zero,
            Self::Constant(value) => AtomSerde::Constant(*value),
            Self::Linear(value) => AtomSerde::Linear(serialize_terms(value)),
            Self::Quadratic(value) => AtomSerde::Quadratic(serialize_terms(value)),
            Self::Polynomial(value) => AtomSerde::Polynomial(serialize_terms(value)),
        };
        value.serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for Atom {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        match AtomSerde::deserialize(deserializer)? {
            AtomSerde::Zero => Ok(Self::Zero),
            AtomSerde::Constant(value) => Ok(Self::Constant(value)),
            AtomSerde::Linear(terms) => crate::Linear::try_from_terms(terms)
                .map(Self::Linear)
                .map_err(serde::de::Error::custom),
            AtomSerde::Quadratic(terms) => crate::Quadratic::try_from_terms(terms)
                .map(Self::Quadratic)
                .map_err(serde::de::Error::custom),
            AtomSerde::Polynomial(terms) => crate::Polynomial::try_from_terms(terms)
                .map(Self::Polynomial)
                .map_err(serde::de::Error::custom),
        }
    }
}

impl Atom {
    pub(crate) fn into_function(self) -> Function {
        match self {
            Self::Zero => Function::Zero,
            Self::Constant(value) => Function::Constant(value),
            Self::Linear(value) => Function::Linear(value),
            Self::Quadratic(value) => Function::Quadratic(value),
            Self::Polynomial(value) => Function::Polynomial(value),
        }
    }

    pub(crate) fn to_function(&self) -> Function {
        self.clone().into_function()
    }

    fn from_function(function: Function) -> Result<Self, Function> {
        match function {
            Function::Zero => Ok(Self::Zero),
            Function::Constant(value) => Ok(Self::Constant(value)),
            Function::Linear(value) => Ok(Self::Linear(value)),
            Function::Quadratic(value) => Ok(Self::Quadratic(value)),
            Function::Polynomial(value) => Ok(Self::Polynomial(value)),
            Function::Expression(expression) => Err(Function::Expression(expression)),
        }
    }
}

/// One instruction in a validated reverse-Polish expression program.
///
/// This is crate-visible only because formatting and the protobuf conversion
/// live outside the operation module. SDK callers construct expressions through
/// [`Function`] operations rather than assembling instructions directly.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "instruction", content = "value", rename_all = "snake_case")]
pub(crate) enum Instruction {
    Push(Atom),
    Unary(UnaryOperator),
    Associative(AssociativeOperator),
    Binary(BinaryOperator),
}

/// A validated non-recursive reverse-Polish expression program.
///
/// The instruction sequence never underflows its value stack and leaves exactly
/// one value. Its fields are private so every construction and deserialization
/// path must pass through the validator.
#[derive(Debug, Clone, PartialEq)]
pub struct Expression {
    instructions: Vec<Instruction>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ExpressionValidationError {
    #[error(
        "expression instruction {instruction_index} requires {required} stack values, found {available}"
    )]
    StackUnderflow {
        instruction_index: usize,
        required: usize,
        available: usize,
    },
    #[error("expression program must leave exactly one stack value, found {height}")]
    InvalidFinalStackHeight { height: usize },
}

impl Expression {
    pub(crate) fn try_from_instructions(
        instructions: Vec<Instruction>,
    ) -> Result<Self, ExpressionValidationError> {
        let mut height = 0usize;
        for (instruction_index, instruction) in instructions.iter().enumerate() {
            let required = match instruction {
                Instruction::Push(_) => {
                    height += 1;
                    continue;
                }
                Instruction::Unary(_) => 1,
                Instruction::Associative(_) | Instruction::Binary(_) => 2,
            };
            if height < required {
                return Err(ExpressionValidationError::StackUnderflow {
                    instruction_index,
                    required,
                    available: height,
                });
            }
            if required == 2 {
                height -= 1;
            }
        }
        if height != 1 {
            return Err(ExpressionValidationError::InvalidFinalStackHeight { height });
        }
        Ok(Self { instructions })
    }

    pub(crate) fn instructions(&self) -> &[Instruction] {
        &self.instructions
    }

    pub(crate) fn into_instructions(self) -> Vec<Instruction> {
        self.instructions
    }
}

impl Function {
    pub(crate) fn from_instructions_exact(
        instructions: Vec<Instruction>,
    ) -> Result<Self, ExpressionValidationError> {
        let expression = Expression::try_from_instructions(instructions)?;
        if expression.instructions.len() == 1 {
            let instruction = expression
                .instructions
                .into_iter()
                .next()
                .expect("length checked");
            let Instruction::Push(atom) = instruction else {
                unreachable!("a valid one-instruction program must push an atom")
            };
            return Ok(atom.into_function());
        }
        Ok(Function::Expression(expression))
    }

    pub(crate) fn into_instructions(self) -> Vec<Instruction> {
        match self {
            Function::Expression(expression) => expression.into_instructions(),
            compact => vec![Instruction::Push(
                Atom::from_function(compact).expect("non-expression Function is a compact atom"),
            )],
        }
    }

    /// Construct an exact unary expression without algebraic folding.
    pub(crate) fn unary_expression(operator: UnaryOperator, operand: Function) -> Self {
        let mut instructions = operand.into_instructions();
        instructions.push(Instruction::Unary(operator));
        Function::from_instructions_exact(instructions)
            .expect("appending a unary instruction preserves expression validity")
    }

    /// Construct an exact associative binary expression without algebraic folding.
    pub(crate) fn associative_expression(
        operator: AssociativeOperator,
        lhs: Function,
        rhs: Function,
    ) -> Self {
        let mut instructions = lhs.into_instructions();
        instructions.extend(rhs.into_instructions());
        instructions.push(Instruction::Associative(operator));
        Function::from_instructions_exact(instructions)
            .expect("combining two valid programs preserves expression validity")
    }

    /// Construct an ordered binary expression without algebraic folding.
    /// Deserializers use this to preserve wire evaluation order and domains.
    pub(crate) fn binary_expression(
        operator: BinaryOperator,
        lhs: Function,
        rhs: Function,
    ) -> Self {
        let mut instructions = lhs.into_instructions();
        instructions.extend(rhs.into_instructions());
        instructions.push(Instruction::Binary(operator));
        Function::from_instructions_exact(instructions)
            .expect("combining two valid programs preserves expression validity")
    }

    pub(crate) fn unary_operation(operator: UnaryOperator, operand: Function) -> Self {
        if operator == UnaryOperator::Powi(1) {
            return operand;
        }

        if let Function::Expression(mut expression) = operand {
            if let Some(Instruction::Unary(inner)) = expression.instructions.last() {
                let inner = *inner;
                if operator == UnaryOperator::Neg && inner == UnaryOperator::Neg {
                    expression.instructions.pop();
                    return Function::from_instructions_exact(expression.instructions)
                        .expect("removing a trailing double negation preserves validity");
                }
                if operator == inner
                    && matches!(operator, UnaryOperator::Abs | UnaryOperator::Signum)
                {
                    return Function::Expression(expression);
                }
            }
            expression.instructions.push(Instruction::Unary(operator));
            return Function::Expression(expression);
        }

        if let Some(value) = operand.as_constant() {
            let value = match operator {
                UnaryOperator::Neg => Some(-value),
                UnaryOperator::Abs => Some(value.abs()),
                UnaryOperator::Signum if value < 0.0 => Some(-1.0),
                UnaryOperator::Signum if value > 0.0 => Some(1.0),
                UnaryOperator::Signum => Some(0.0),
                UnaryOperator::Powi(exponent) if value == 0.0 && exponent < 0 => None,
                UnaryOperator::Powi(exponent) => Some(value.powi(exponent)),
            };
            if let Some(value) = value.filter(|value| value.is_finite()) {
                return Function::try_from(value).expect("value was checked as finite");
            }
        }

        if operator == UnaryOperator::Neg && operand.is_polynomial() {
            return -operand;
        }

        Function::unary_expression(operator, operand)
    }

    pub(crate) fn associative_operation(
        operator: AssociativeOperator,
        lhs: Function,
        rhs: Function,
    ) -> Self {
        if matches!(
            operator,
            AssociativeOperator::Min | AssociativeOperator::Max
        ) {
            if let (Some(lhs), Some(rhs)) = (lhs.as_constant(), rhs.as_constant()) {
                let value = match operator {
                    AssociativeOperator::Min => lhs.min(rhs),
                    AssociativeOperator::Max => lhs.max(rhs),
                    _ => unreachable!(),
                };
                return Function::try_from(value)
                    .expect("minimum/maximum of finite values is finite");
            }
        }
        Function::associative_expression(operator, lhs, rhs)
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
        Self::associative_operation(AssociativeOperator::Min, self, rhs)
    }

    /// Pointwise maximum of two functions.
    pub fn max(self, rhs: Self) -> Self {
        Self::associative_operation(AssociativeOperator::Max, self, rhs)
    }

    /// Raise this function to an integer power.
    ///
    /// The exponent is an operation parameter rather than another [`Function`].
    /// Apart from identity and constant folding, the resulting expression
    /// remains composed and therefore has no compact polynomial degree, even
    /// for non-negative exponents.
    pub fn powi(self, exponent: i32) -> Self {
        Self::unary_operation(UnaryOperator::Powi(exponent), self)
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
    fn min_max_preserve_operand_order_as_binary_instructions() {
        let x = Function::from(linear!(1));
        let expression = x.clone().min(constant(2.0)).min(constant(-1.0));
        let Function::Expression(program) = &expression else {
            panic!("minimum should remain a composed expression");
        };
        assert_eq!(program.instructions().len(), 5);
        assert!(matches!(
            program.instructions()[2],
            Instruction::Associative(AssociativeOperator::Min)
        ));
        assert!(matches!(
            program.instructions()[4],
            Instruction::Associative(AssociativeOperator::Min)
        ));
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
    fn associative_operations_preserve_right_grouping() {
        let a = Function::from(linear!(1)).abs();
        let b = Function::from(linear!(2)).abs();
        let c = Function::from(linear!(3)).abs();

        let left_grouped = ((a.clone() + b.clone()).unwrap() + c.clone()).unwrap();
        let right_grouped = (a + (b + c).unwrap()).unwrap();

        let Function::Expression(left) = &left_grouped else {
            panic!("composite addition should use an expression program");
        };
        let Function::Expression(right) = &right_grouped else {
            panic!("composite addition should use an expression program");
        };
        assert!(matches!(
            left.instructions()[4],
            Instruction::Associative(AssociativeOperator::Add)
        ));
        assert!(matches!(
            *left.instructions().last().unwrap(),
            Instruction::Associative(AssociativeOperator::Add)
        ));
        assert!(matches!(
            right.instructions()[6],
            Instruction::Associative(AssociativeOperator::Add)
        ));
        assert!(matches!(
            *right.instructions().last().unwrap(),
            Instruction::Associative(AssociativeOperator::Add)
        ));
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
            let Function::Expression(program) = expression else {
                panic!("coefficient/composite arithmetic should use an expression program");
            };
            assert!(matches!(
                program.instructions().first(),
                Some(Instruction::Push(Atom::Constant(value))) if value.into_inner() == 2.0
            ));
        }
    }

    #[test]
    fn arithmetic_is_closed_over_composite_functions() {
        let x = Function::from(linear!(1));
        let absolute = x.clone().abs();
        let sum = (absolute.clone() + constant(1.0)).unwrap();
        let product = (x.clone() * absolute.clone()).unwrap();
        let negated = -absolute;

        assert!(matches!(sum, Function::Expression(_)));
        assert!(matches!(product, Function::Expression(_)));
        assert!(matches!(negated, Function::Expression(_)));
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
    fn division_and_integer_power_report_undefined_values() {
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

        let square = x.clone().powi(2);
        assert!(matches!(square, Function::Expression(_)));
        assert_eq!(square.degree(), None);
        assert_eq!(square.evaluate(&state(-3.0), ATol::default()).unwrap(), 9.0);

        let inverse_square = x.powi(-2);
        assert_eq!(
            inverse_square
                .evaluate(&state(2.0), ATol::default())
                .unwrap(),
            0.25
        );
        let error = inverse_square
            .evaluate(&state(0.0), ATol::default())
            .unwrap_err();
        assert!(matches!(
            error.downcast_ref::<FunctionEvaluationError>(),
            Some(FunctionEvaluationError::ZeroToNegativeIntegerPower { exponent: -2 })
        ));

        assert_eq!(
            constant(0.0)
                .powi(0)
                .evaluate(&crate::v1::State::default(), ATol::default())
                .unwrap(),
            1.0
        );

        let identity = Function::from(linear!(1)).powi(1);
        assert!(identity.is_polynomial());
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
        let expression = (Function::zero() * reciprocal.clone()).unwrap();
        assert!(matches!(expression, Function::Expression(_)));
        assert!(expression.evaluate(&state(0.0), ATol::default()).is_err());

        let zero_power = reciprocal.powi(0);
        assert!(matches!(zero_power, Function::Expression(_)));
        assert!(zero_power.evaluate(&state(0.0), ATol::default()).is_err());
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
    fn appending_to_an_explicit_rpn_program_never_eagerly_expands_it() {
        let scaled_x = Function::from((coeff!(1e150) * linear!(1)).unwrap());
        let explicit =
            Function::associative_expression(AssociativeOperator::Mul, scaled_x.clone(), scaled_x);
        let appended = (explicit * constant(0.5)).unwrap();
        let Function::Expression(program) = &appended else {
            panic!("appending to an explicit expression must preserve the program");
        };
        assert_eq!(program.instructions().len(), 5);
        let state = state(1e-200);
        let operand: f64 = 1e150 * 1e-200;
        assert_eq!(
            appended
                .evaluate(&state, ATol::default())
                .unwrap()
                .to_bits(),
            (operand * operand * 0.5).to_bits(),
        );

        let product_of_sums = (1..=20)
            .map(|id| {
                (Function::from(linear!(id)) + constant(1.0)).expect("polynomial addition is valid")
            })
            .reduce(|lhs, rhs| Function::associative_expression(AssociativeOperator::Mul, lhs, rhs))
            .unwrap();
        let appended = (product_of_sums * constant(2.0)).unwrap();
        let Function::Expression(program) = appended else {
            panic!("explicit product-of-sums must not be expanded");
        };
        assert_eq!(program.instructions().len(), 41);
    }
}
