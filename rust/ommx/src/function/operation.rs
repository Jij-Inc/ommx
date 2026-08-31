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
pub enum UnaryOperator {
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
pub enum AssociativeOperator {
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
pub enum BinaryOperator {
    Div,
}

/// A non-recursive compact-polynomial operand in an expression program.
#[derive(Debug, Clone, PartialEq)]
pub enum Atom {
    Zero,
    Constant(Coefficient),
    Linear(crate::Linear),
    Quadratic(crate::Quadratic),
    Polynomial(crate::Polynomial),
}

#[derive(serde::Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
enum AtomSerde {
    Zero,
    Constant(Coefficient),
    Linear(Vec<(crate::LinearMonomial, Coefficient)>),
    Quadratic(Vec<(crate::QuadraticMonomial, Coefficient)>),
    Polynomial(Vec<(crate::MonomialDyn, Coefficient)>),
}

struct SerializeTerms<'a, M: crate::Monomial>(&'a crate::PolynomialBase<M>);

impl<M> serde::Serialize for SerializeTerms<'_, M>
where
    M: crate::Monomial + serde::Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeSeq;

        let mut sequence = serializer.serialize_seq(Some(self.0.num_terms()))?;
        for (monomial, coefficient) in self.0.iter() {
            sequence.serialize_element(&(monomial, coefficient))?;
        }
        sequence.end()
    }
}

#[derive(serde::Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
enum AtomSerialize<'a> {
    Zero,
    Constant(Coefficient),
    Linear(SerializeTerms<'a, crate::LinearMonomial>),
    Quadratic(SerializeTerms<'a, crate::QuadraticMonomial>),
    Polynomial(SerializeTerms<'a, crate::MonomialDyn>),
}

impl serde::Serialize for Atom {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let value = match self {
            Self::Zero => AtomSerialize::Zero,
            Self::Constant(value) => AtomSerialize::Constant(*value),
            Self::Linear(value) => AtomSerialize::Linear(SerializeTerms(value)),
            Self::Quadratic(value) => AtomSerialize::Quadratic(SerializeTerms(value)),
            Self::Polynomial(value) => AtomSerialize::Polynomial(SerializeTerms(value)),
        };
        serde::Serialize::serialize(&value, serializer)
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
    pub fn into_function(self) -> Function {
        match self {
            Self::Zero => Function::Zero,
            Self::Constant(value) => Function::Constant(value),
            Self::Linear(value) => Function::Linear(value),
            Self::Quadratic(value) => Function::Quadratic(value),
            Self::Polynomial(value) => Function::Polynomial(value),
        }
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

/// Borrowed view of a monomial stored in a compact polynomial.
///
/// The view keeps dynamic monomials borrowed even when their degree exceeds
/// the inline capacity of [`crate::MonomialDyn`].
#[derive(Clone, Copy)]
pub(crate) enum MonomialRef<'a> {
    Constant,
    Linear(&'a crate::LinearMonomial),
    Quadratic(&'a crate::QuadraticMonomial),
    Polynomial(&'a crate::MonomialDyn),
}

impl<'a> MonomialRef<'a> {
    fn as_linear(self) -> Option<crate::LinearMonomial> {
        match self {
            Self::Constant => Some(crate::LinearMonomial::Constant),
            Self::Linear(monomial) => Some(*monomial),
            Self::Quadratic(monomial) => crate::LinearMonomial::try_from(monomial).ok(),
            Self::Polynomial(monomial) => crate::LinearMonomial::try_from(monomial).ok(),
        }
    }

    fn as_quadratic(self) -> Option<crate::QuadraticMonomial> {
        match self {
            Self::Constant => Some(crate::QuadraticMonomial::Constant),
            Self::Linear(monomial) => Some((*monomial).into()),
            Self::Quadratic(monomial) => Some(*monomial),
            Self::Polynomial(monomial) => crate::QuadraticMonomial::try_from(monomial).ok(),
        }
    }

    fn with_dynamic<R>(self, get: impl FnOnce(&crate::MonomialDyn) -> R) -> R {
        match self {
            Self::Constant => get(&crate::MonomialDyn::default()),
            Self::Linear(monomial) => get(&(*monomial).into()),
            Self::Quadratic(monomial) => get(&(*monomial).into()),
            Self::Polynomial(monomial) => get(monomial),
        }
    }

    pub(crate) fn len(self) -> usize {
        match self {
            Self::Constant => 0,
            Self::Linear(monomial) => monomial.iter().count(),
            Self::Quadratic(monomial) => monomial.iter().count(),
            Self::Polynomial(monomial) => monomial.len(),
        }
    }

    pub(crate) fn ids(self) -> impl Iterator<Item = crate::VariableID> + 'a {
        MonomialIds {
            monomial: self,
            index: 0,
        }
    }
}

struct MonomialIds<'a> {
    monomial: MonomialRef<'a>,
    index: usize,
}

impl Iterator for MonomialIds<'_> {
    type Item = crate::VariableID;

    fn next(&mut self) -> Option<Self::Item> {
        let id = match self.monomial {
            MonomialRef::Constant => None,
            MonomialRef::Linear(crate::LinearMonomial::Constant) => None,
            MonomialRef::Linear(crate::LinearMonomial::Variable(id)) => {
                (self.index == 0).then_some(*id)
            }
            MonomialRef::Quadratic(crate::QuadraticMonomial::Constant) => None,
            MonomialRef::Quadratic(crate::QuadraticMonomial::Linear(id)) => {
                (self.index == 0).then_some(*id)
            }
            MonomialRef::Quadratic(crate::QuadraticMonomial::Pair(pair)) => match self.index {
                0 => Some(pair.lower()),
                1 => Some(pair.upper()),
                _ => None,
            },
            MonomialRef::Polynomial(monomial) => monomial.get(self.index).copied(),
        };
        self.index += 1;
        id
    }
}

/// Borrowed view of a compact polynomial [`Function`] or expression [`Atom`].
///
/// Comparison and formatting cross the `Function`/expression boundary but only
/// need read access to the stored coefficient map. Keeping this view
/// crate-private lets those paths share variant dispatch without constructing
/// an owned `Function` or exposing expression internals through the SDK API.
#[derive(Clone, Copy)]
pub(crate) enum PolynomialRef<'a> {
    Zero,
    Constant(&'a Coefficient),
    Linear(&'a crate::Linear),
    Quadratic(&'a crate::Quadratic),
    Polynomial(&'a crate::Polynomial),
}

impl<'a> PolynomialRef<'a> {
    pub(crate) fn from_function(function: &'a Function) -> Option<Self> {
        match function {
            Function::Zero => Some(Self::Zero),
            Function::Constant(value) => Some(Self::Constant(value)),
            Function::Linear(value) => Some(Self::Linear(value)),
            Function::Quadratic(value) => Some(Self::Quadratic(value)),
            Function::Polynomial(value) => Some(Self::Polynomial(value)),
            Function::Expression(_) => None,
        }
    }

    pub(crate) fn from_atom(atom: &'a Atom) -> Self {
        match atom {
            Atom::Zero => Self::Zero,
            Atom::Constant(value) => Self::Constant(value),
            Atom::Linear(value) => Self::Linear(value),
            Atom::Quadratic(value) => Self::Quadratic(value),
            Atom::Polynomial(value) => Self::Polynomial(value),
        }
    }

    pub(crate) fn num_terms(self) -> usize {
        match self {
            Self::Zero => 0,
            Self::Constant(_) => 1,
            Self::Linear(value) => value.num_terms(),
            Self::Quadratic(value) => value.num_terms(),
            Self::Polynomial(value) => value.num_terms(),
        }
    }

    pub(crate) fn coefficient(self, monomial: MonomialRef<'_>) -> Option<Coefficient> {
        match self {
            Self::Zero => None,
            Self::Constant(value) => (monomial.len() == 0).then_some(*value),
            Self::Linear(value) => monomial
                .as_linear()
                .and_then(|monomial| value.get(&monomial)),
            Self::Quadratic(value) => monomial
                .as_quadratic()
                .and_then(|monomial| value.get(&monomial)),
            Self::Polynomial(value) => monomial.with_dynamic(|monomial| value.get(monomial)),
        }
    }

    pub(crate) fn all_terms(
        self,
        mut predicate: impl FnMut(MonomialRef<'a>, Coefficient) -> bool,
    ) -> bool {
        match self {
            Self::Zero => true,
            Self::Constant(value) => predicate(MonomialRef::Constant, *value),
            Self::Linear(value) => value.iter().all(|(monomial, coefficient)| {
                predicate(MonomialRef::Linear(monomial), *coefficient)
            }),
            Self::Quadratic(value) => value.iter().all(|(monomial, coefficient)| {
                predicate(MonomialRef::Quadratic(monomial), *coefficient)
            }),
            Self::Polynomial(value) => value.iter().all(|(monomial, coefficient)| {
                predicate(MonomialRef::Polynomial(monomial), *coefficient)
            }),
        }
    }

    pub(crate) fn for_each_term(self, mut visit: impl FnMut(MonomialRef<'a>, Coefficient)) {
        let completed = self.all_terms(|monomial, coefficient| {
            visit(monomial, coefficient);
            true
        });
        debug_assert!(completed);
    }
}

/// One instruction in a validated reverse-Polish expression program.
///
/// The operation module owns instruction construction and validation. SDK
/// callers construct expressions through [`Function`] operations rather than
/// assembling instructions directly.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "instruction", content = "value", rename_all = "snake_case")]
pub enum Instruction {
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
pub enum ExpressionValidationError {
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

fn try_from_instructions(
    instructions: Vec<Instruction>,
) -> Result<Expression, ExpressionValidationError> {
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
    Ok(Expression { instructions })
}

pub fn instructions(expression: &Expression) -> &[Instruction] {
    &expression.instructions
}

pub fn into_expression_instructions(expression: Expression) -> Vec<Instruction> {
    expression.instructions
}

pub fn from_instructions_exact(
    instructions: Vec<Instruction>,
) -> Result<Function, ExpressionValidationError> {
    let expression = try_from_instructions(instructions)?;
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

pub fn into_instructions(function: Function) -> Vec<Instruction> {
    match function {
        Function::Expression(expression) => into_expression_instructions(expression),
        compact => vec![Instruction::Push(
            Atom::from_function(compact).expect("non-expression Function is a compact atom"),
        )],
    }
}

/// Construct an exact unary expression without algebraic folding.
pub fn unary_expression(operator: UnaryOperator, operand: Function) -> Function {
    let mut instructions = into_instructions(operand);
    instructions.push(Instruction::Unary(operator));
    from_instructions_exact(instructions)
        .expect("appending a unary instruction preserves expression validity")
}

/// Construct an exact associative binary expression without algebraic folding.
pub fn associative_expression(
    operator: AssociativeOperator,
    lhs: Function,
    rhs: Function,
) -> Function {
    let mut instructions = into_instructions(lhs);
    instructions.extend(into_instructions(rhs));
    instructions.push(Instruction::Associative(operator));
    from_instructions_exact(instructions)
        .expect("combining two valid programs preserves expression validity")
}

/// Construct an ordered binary expression without algebraic folding.
/// Deserializers use this to preserve wire evaluation order and domains.
pub fn binary_expression(operator: BinaryOperator, lhs: Function, rhs: Function) -> Function {
    let mut instructions = into_instructions(lhs);
    instructions.extend(into_instructions(rhs));
    instructions.push(Instruction::Binary(operator));
    from_instructions_exact(instructions)
        .expect("combining two valid programs preserves expression validity")
}

pub fn unary_operation(operator: UnaryOperator, operand: Function) -> Function {
    if operator == UnaryOperator::Powi(1) {
        return operand;
    }

    if let Function::Expression(mut expression) = operand {
        if let Some(Instruction::Unary(inner)) = expression.instructions.last() {
            let inner = *inner;
            if operator == UnaryOperator::Neg && inner == UnaryOperator::Neg {
                expression.instructions.pop();
                return from_instructions_exact(expression.instructions)
                    .expect("removing a trailing double negation preserves validity");
            }
            if operator == inner && operator == UnaryOperator::Abs {
                return Function::Expression(expression);
            }
        }
        expression.instructions.push(Instruction::Unary(operator));
        return Function::Expression(expression);
    }

    if let Some(value) = as_constant(&operand) {
        let value = match operator {
            UnaryOperator::Neg => Some(-value),
            UnaryOperator::Abs => Some(value.abs()),
            UnaryOperator::Signum => None,
            UnaryOperator::Powi(exponent) if exponent < 0 => None,
            UnaryOperator::Powi(exponent) => Some(value.powi(exponent)),
        };
        if let Some(value) = value.filter(|value| value.is_finite()) {
            return Function::try_from(value).expect("value was checked as finite");
        }
    }

    if operator == UnaryOperator::Neg && operand.is_polynomial() {
        return -operand;
    }

    unary_expression(operator, operand)
}

pub fn associative_operation(
    operator: AssociativeOperator,
    lhs: Function,
    rhs: Function,
) -> Function {
    if matches!(
        operator,
        AssociativeOperator::Min | AssociativeOperator::Max
    ) {
        if let (Some(lhs), Some(rhs)) = (as_constant(&lhs), as_constant(&rhs)) {
            let value = match operator {
                AssociativeOperator::Min => lhs.min(rhs),
                AssociativeOperator::Max => lhs.max(rhs),
                _ => unreachable!(),
            };
            return Function::try_from(value).expect("minimum/maximum of finite values is finite");
        }
    }
    associative_expression(operator, lhs, rhs)
}

pub fn binary_operation(operator: BinaryOperator, lhs: Function, rhs: Function) -> Function {
    binary_expression(operator, lhs, rhs)
}

pub fn as_constant(function: &Function) -> Option<f64> {
    match function {
        Function::Zero => Some(0.0),
        Function::Constant(value) => Some(value.into_inner()),
        Function::Linear(value) if value.degree().into_inner() == 0 => Some(value.constant_term()),
        Function::Quadratic(value) if value.degree().into_inner() == 0 => {
            Some(value.constant_term())
        }
        Function::Polynomial(value) if value.degree().into_inner() == 0 => {
            Some(value.constant_term())
        }
        _ => None,
    }
}

impl Function {
    /// Absolute value of this function.
    pub fn abs(self) -> Self {
        unary_operation(UnaryOperator::Abs, self)
    }

    /// Mathematical sign function.
    ///
    /// Evaluation returns zero when the absolute value of the operand is less
    /// than or equal to the caller-provided [`crate::ATol`].
    pub fn signum(self) -> Self {
        unary_operation(UnaryOperator::Signum, self)
    }

    /// Pointwise minimum of two functions.
    pub fn min(self, rhs: Self) -> Self {
        associative_operation(AssociativeOperator::Min, self, rhs)
    }

    /// Pointwise maximum of two functions.
    pub fn max(self, rhs: Self) -> Self {
        associative_operation(AssociativeOperator::Max, self, rhs)
    }

    /// Raise this function to an integer power.
    ///
    /// The exponent is an operation parameter rather than another [`Function`].
    /// Apart from identity and constant folding, the resulting expression
    /// remains composed and therefore has no compact polynomial degree, even
    /// for non-negative exponents. With a negative exponent, evaluation fails
    /// when the absolute value of the operand is less than or equal to the
    /// caller-provided [`crate::ATol`].
    pub fn powi(self, exponent: i32) -> Self {
        unary_operation(UnaryOperator::Powi(exponent), self)
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
    fn zero_sensitive_constant_operations_remain_composed_until_evaluation() {
        let tiny = constant(1e-8);
        let signum = tiny.clone().signum();
        let reciprocal = (Function::one() / tiny.clone()).unwrap();
        let inverse = tiny.powi(-1);

        assert!(matches!(&signum, Function::Expression(_)));
        assert!(matches!(&reciprocal, Function::Expression(_)));
        assert!(matches!(&inverse, Function::Expression(_)));

        let state = crate::v1::State::default();
        let coarse = ATol::new(1e-6).unwrap();
        let fine = ATol::new(1e-9).unwrap();
        assert_eq!(signum.evaluate(&state, coarse).unwrap(), 0.0);
        assert_eq!(signum.evaluate(&state, fine).unwrap(), 1.0);

        let error = reciprocal.evaluate(&state, coarse).unwrap_err();
        assert!(matches!(
            error.downcast_ref::<FunctionEvaluationError>(),
            Some(FunctionEvaluationError::DivisionByZero)
        ));
        assert_eq!(reciprocal.evaluate(&state, fine).unwrap(), 1e8);

        let error = inverse.evaluate(&state, coarse).unwrap_err();
        assert!(matches!(
            error.downcast_ref::<FunctionEvaluationError>(),
            Some(FunctionEvaluationError::ZeroToNegativeIntegerPower { exponent: -1 })
        ));
        assert_eq!(inverse.evaluate(&state, fine).unwrap(), 1e8);
    }

    #[test]
    fn nested_signum_is_not_simplified_across_tolerance_boundaries() {
        let nested = Function::from(linear!(1)).signum().signum();
        let Function::Expression(program) = &nested else {
            panic!("nested signum should remain a composed expression");
        };
        assert_eq!(
            instructions(program)
                .iter()
                .filter(|instruction| matches!(
                    instruction,
                    Instruction::Unary(UnaryOperator::Signum)
                ))
                .count(),
            2
        );

        // signum(3) is 1, which the outer signum classifies as zero at this tolerance.
        assert_eq!(
            nested
                .evaluate(&state(3.0), ATol::new(2.0).unwrap())
                .unwrap(),
            0.0
        );
    }

    #[test]
    fn min_max_preserve_operand_order_as_binary_instructions() {
        let x = Function::from(linear!(1));
        let expression = x.clone().min(constant(2.0)).min(constant(-1.0));
        let Function::Expression(program) = &expression else {
            panic!("minimum should remain a composed expression");
        };
        assert_eq!(instructions(program).len(), 5);
        assert!(matches!(
            instructions(program)[2],
            Instruction::Associative(AssociativeOperator::Min)
        ));
        assert!(matches!(
            instructions(program)[4],
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
            instructions(left)[4],
            Instruction::Associative(AssociativeOperator::Add)
        ));
        assert!(matches!(
            *instructions(left).last().unwrap(),
            Instruction::Associative(AssociativeOperator::Add)
        ));
        assert!(matches!(
            instructions(right)[6],
            Instruction::Associative(AssociativeOperator::Add)
        ));
        assert!(matches!(
            *instructions(right).last().unwrap(),
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
                instructions(&program).first(),
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
    fn division_by_a_coefficient_remains_composed_until_evaluation() {
        let x = Function::from(linear!(1));
        let divided = (x / coeff!(2.0)).unwrap();
        assert!(matches!(&divided, Function::Expression(_)));
        assert_eq!(
            divided
                .evaluate(&state(4.0), ATol::new(1.0).unwrap())
                .unwrap(),
            2.0
        );

        let error = divided
            .evaluate(&state(4.0), ATol::new(2.0).unwrap())
            .unwrap_err();
        assert!(matches!(
            error.downcast_ref::<FunctionEvaluationError>(),
            Some(FunctionEvaluationError::DivisionByZero)
        ));
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
        assert_eq!(as_constant(&expression), Some(3.0));

        let substituted = x
            .signum()
            .substitute_one(1.into(), &constant(-4.0))
            .unwrap();
        assert!(matches!(&substituted, Function::Expression(_)));
        assert_eq!(
            substituted
                .evaluate(&crate::v1::State::default(), ATol::new(1.0).unwrap())
                .unwrap(),
            -1.0
        );
        assert_eq!(
            substituted
                .evaluate(&crate::v1::State::default(), ATol::new(5.0).unwrap())
                .unwrap(),
            0.0
        );
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
        let explicit = associative_expression(AssociativeOperator::Mul, scaled_x.clone(), scaled_x);
        let appended = (explicit * constant(0.5)).unwrap();
        let Function::Expression(program) = &appended else {
            panic!("appending to an explicit expression must preserve the program");
        };
        assert_eq!(instructions(program).len(), 5);
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
            .reduce(|lhs, rhs| associative_expression(AssociativeOperator::Mul, lhs, rhs))
            .unwrap();
        let appended = (product_of_sums * constant(2.0)).unwrap();
        let Function::Expression(program) = appended else {
            panic!("explicit product-of-sums must not be expanded");
        };
        assert_eq!(instructions(&program).len(), 41);
    }
}
