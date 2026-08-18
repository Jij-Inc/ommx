use super::operation::{
    from_instructions_exact, into_expression_instructions, AssociativeOperator, Atom,
    BinaryOperator, Instruction, UnaryOperator,
};
use super::*;
use crate::{
    parse::{Parse, ParseError, RawParseError},
    v1, CoefficientError,
};

const FUNCTION_MESSAGE: &str = "ommx.v1.Function";
const EXPRESSION_MESSAGE: &str = "ommx.v1.Function.Expression";
const INSTRUCTION_MESSAGE: &str = "ommx.v1.Function.Expression.Instruction";
const UNARY_MESSAGE: &str = "ommx.v1.Function.Expression.Instruction.UnaryOperation";
const ASSOCIATIVE_MESSAGE: &str = "ommx.v1.Function.Expression.Instruction.AssociativeOperation";
const BINARY_MESSAGE: &str = "ommx.v1.Function.Expression.Instruction.BinaryOperation";

/// Note for [`Parse`] implementation
/// ----------------------------------
/// Since the `ommx.v1.Function` is defined by `oneof` in protobuf,
/// it may be `None` if we extend the `Function` enum in the future.
/// Suppose that we add new entry to `ommx.v1.Function`, e.g. `Exponential` or `Logarithm`,
/// and save it as `ommx.v1.Function` in future version of OMMX SDK. This encoded message may be decoded
/// by the current version of OMMX SDK, which does not support the new entry.
/// In this case, the new entry is decoded as `None`.
impl Parse for v1::Function {
    type Output = Function;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        use v1::function::Function::*;
        match self.function.ok_or(RawParseError::UnsupportedV1Function)? {
            Constant(value) => {
                parse_constant(value, FUNCTION_MESSAGE, "constant").map(Atom::into_function)
            }
            Linear(value) => Ok(Function::Linear(value.parse_as(
                &(),
                FUNCTION_MESSAGE,
                "linear",
            )?)),
            Quadratic(value) => Ok(Function::Quadratic(value.parse_as(
                &(),
                FUNCTION_MESSAGE,
                "quadratic",
            )?)),
            Polynomial(value) => Ok(Function::Polynomial(value.parse_as(
                &(),
                FUNCTION_MESSAGE,
                "polynomial",
            )?)),
            Expression(value) => value.parse_as(&(), FUNCTION_MESSAGE, "expression"),
        }
    }
}

impl Parse for v1::function::Expression {
    type Output = Function;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let instructions = self
            .instructions
            .into_iter()
            .map(|instruction| {
                parse_instruction(instruction)
                    .map_err(|error| error.context(EXPRESSION_MESSAGE, "instructions"))
            })
            .collect::<Result<Vec<_>, _>>()?;

        from_instructions_exact(instructions).map_err(|error| {
            RawParseError::InvalidFunction(error.to_string())
                .context(EXPRESSION_MESSAGE, "instructions")
        })
    }
}

fn parse_instruction(
    value: v1::function::expression::Instruction,
) -> Result<Instruction, ParseError> {
    use v1::function::expression::instruction::Instruction::*;
    match value.instruction.ok_or(RawParseError::MissingField {
        message: INSTRUCTION_MESSAGE,
        field: "instruction",
    })? {
        Constant(value) => {
            parse_constant(value, INSTRUCTION_MESSAGE, "constant").map(Instruction::Push)
        }
        Linear(value) => Ok(Instruction::Push(Atom::Linear(value.parse_as(
            &(),
            INSTRUCTION_MESSAGE,
            "linear",
        )?))),
        Quadratic(value) => Ok(Instruction::Push(Atom::Quadratic(value.parse_as(
            &(),
            INSTRUCTION_MESSAGE,
            "quadratic",
        )?))),
        Polynomial(value) => Ok(Instruction::Push(Atom::Polynomial(value.parse_as(
            &(),
            INSTRUCTION_MESSAGE,
            "polynomial",
        )?))),
        Unary(value) => {
            parse_unary(value).map_err(|error| error.context(INSTRUCTION_MESSAGE, "unary"))
        }
        Associative(value) => parse_associative(value)
            .map_err(|error| error.context(INSTRUCTION_MESSAGE, "associative")),
        Binary(value) => {
            parse_binary(value).map_err(|error| error.context(INSTRUCTION_MESSAGE, "binary"))
        }
    }
}

fn parse_unary(
    value: v1::function::expression::instruction::UnaryOperation,
) -> Result<Instruction, ParseError> {
    use v1::function::expression::instruction::unary_operation::Operator;
    let operator = match Operator::try_from(value.operator) {
        Ok(Operator::Neg) => UnaryOperator::Neg,
        Ok(Operator::Abs) => UnaryOperator::Abs,
        Ok(Operator::Signum) => UnaryOperator::Signum,
        Ok(Operator::Powi) => {
            UnaryOperator::Powi(value.integer_exponent.ok_or(RawParseError::MissingField {
                message: UNARY_MESSAGE,
                field: "integer_exponent",
            })?)
        }
        Ok(Operator::Unspecified) | Err(_) => {
            return Err(RawParseError::UnknownEnumValue {
                enum_name: "ommx.v1.Function.Expression.Instruction.UnaryOperation.Operator",
                value: value.operator,
            }
            .context(UNARY_MESSAGE, "operator"));
        }
    };
    if !matches!(operator, UnaryOperator::Powi(_)) && value.integer_exponent.is_some() {
        return Err(RawParseError::InvalidFunction(
            "integer_exponent is only valid for unary POWI instructions".into(),
        )
        .context(UNARY_MESSAGE, "integer_exponent"));
    }
    Ok(Instruction::Unary(operator))
}

fn parse_associative(
    value: v1::function::expression::instruction::AssociativeOperation,
) -> Result<Instruction, ParseError> {
    use v1::function::expression::instruction::associative_operation::Operator;
    let operator = match Operator::try_from(value.operator) {
        Ok(Operator::Add) => AssociativeOperator::Add,
        Ok(Operator::Mul) => AssociativeOperator::Mul,
        Ok(Operator::Min) => AssociativeOperator::Min,
        Ok(Operator::Max) => AssociativeOperator::Max,
        Ok(Operator::Unspecified) | Err(_) => {
            return Err(RawParseError::UnknownEnumValue {
                enum_name: "ommx.v1.Function.Expression.Instruction.AssociativeOperation.Operator",
                value: value.operator,
            }
            .context(ASSOCIATIVE_MESSAGE, "operator"));
        }
    };
    Ok(Instruction::Associative(operator))
}

fn parse_binary(
    value: v1::function::expression::instruction::BinaryOperation,
) -> Result<Instruction, ParseError> {
    use v1::function::expression::instruction::binary_operation::Operator;
    let operator = match Operator::try_from(value.operator) {
        Ok(Operator::Div) => BinaryOperator::Div,
        Ok(Operator::Unspecified) | Err(_) => {
            return Err(RawParseError::UnknownEnumValue {
                enum_name: "ommx.v1.Function.Expression.Instruction.BinaryOperation.Operator",
                value: value.operator,
            }
            .context(BINARY_MESSAGE, "operator"));
        }
    };
    Ok(Instruction::Binary(operator))
}

fn parse_constant(
    value: f64,
    message: &'static str,
    field: &'static str,
) -> Result<Atom, ParseError> {
    match value.try_into() {
        Ok(value) => Ok(Atom::Constant(value)),
        Err(CoefficientError::Zero) => Ok(Atom::Zero),
        Err(error) => Err(RawParseError::from(error).context(message, field)),
    }
}

impl TryFrom<v1::Function> for Function {
    type Error = ParseError;

    fn try_from(value: v1::Function) -> Result<Self, Self::Error> {
        value.parse(&())
    }
}

impl From<Function> for v1::Function {
    fn from(value: Function) -> Self {
        use v1::function::Function as WireFunction;
        let function = match value {
            Function::Zero => WireFunction::Constant(0.0),
            Function::Constant(value) => WireFunction::Constant(value.into()),
            Function::Linear(value) => WireFunction::Linear(value.into()),
            Function::Quadratic(value) => WireFunction::Quadratic(value.into()),
            Function::Polynomial(value) => WireFunction::Polynomial(value.into()),
            Function::Expression(expression) => {
                WireFunction::Expression(v1::function::Expression {
                    instructions: into_expression_instructions(expression)
                        .into_iter()
                        .map(Into::into)
                        .collect(),
                })
            }
        };
        Self {
            function: Some(function),
        }
    }
}

impl From<Instruction> for v1::function::expression::Instruction {
    fn from(value: Instruction) -> Self {
        use v1::function::expression::instruction::{
            associative_operation, binary_operation, unary_operation, AssociativeOperation,
            BinaryOperation, Instruction as WireInstruction, UnaryOperation,
        };
        let instruction = match value {
            Instruction::Push(Atom::Zero) => WireInstruction::Constant(0.0),
            Instruction::Push(Atom::Constant(value)) => WireInstruction::Constant(value.into()),
            Instruction::Push(Atom::Linear(value)) => WireInstruction::Linear(value.into()),
            Instruction::Push(Atom::Quadratic(value)) => WireInstruction::Quadratic(value.into()),
            Instruction::Push(Atom::Polynomial(value)) => WireInstruction::Polynomial(value.into()),
            Instruction::Unary(operator) => {
                let (operator, integer_exponent) = match operator {
                    UnaryOperator::Neg => (unary_operation::Operator::Neg as i32, None),
                    UnaryOperator::Abs => (unary_operation::Operator::Abs as i32, None),
                    UnaryOperator::Signum => (unary_operation::Operator::Signum as i32, None),
                    UnaryOperator::Powi(exponent) => {
                        (unary_operation::Operator::Powi as i32, Some(exponent))
                    }
                };
                WireInstruction::Unary(UnaryOperation {
                    operator,
                    integer_exponent,
                })
            }
            Instruction::Associative(operator) => {
                let operator = match operator {
                    AssociativeOperator::Add => associative_operation::Operator::Add,
                    AssociativeOperator::Mul => associative_operation::Operator::Mul,
                    AssociativeOperator::Min => associative_operation::Operator::Min,
                    AssociativeOperator::Max => associative_operation::Operator::Max,
                };
                WireInstruction::Associative(AssociativeOperation {
                    operator: operator as i32,
                })
            }
            Instruction::Binary(BinaryOperator::Div) => WireInstruction::Binary(BinaryOperation {
                operator: binary_operation::Operator::Div as i32,
            }),
        };
        Self {
            instruction: Some(instruction),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FunctionParameters, Message, PolynomialParameters};
    use proptest::prelude::*;
    use v1::function::expression::instruction::{
        associative_operation, binary_operation, unary_operation, AssociativeOperation,
        BinaryOperation, Instruction as WireInstruction, UnaryOperation,
    };

    fn instruction(instruction: WireInstruction) -> v1::function::expression::Instruction {
        v1::function::expression::Instruction {
            instruction: Some(instruction),
        }
    }

    fn expression(instructions: Vec<WireInstruction>) -> v1::Function {
        v1::Function {
            function: Some(v1::function::Function::Expression(
                v1::function::Expression {
                    instructions: instructions.into_iter().map(instruction).collect(),
                },
            )),
        }
    }

    fn unary(operator: unary_operation::Operator) -> WireInstruction {
        WireInstruction::Unary(UnaryOperation {
            operator: operator as i32,
            integer_exponent: None,
        })
    }

    fn powi(exponent: Option<i32>) -> WireInstruction {
        WireInstruction::Unary(UnaryOperation {
            operator: unary_operation::Operator::Powi as i32,
            integer_exponent: exponent,
        })
    }

    fn associative(operator: associative_operation::Operator) -> WireInstruction {
        WireInstruction::Associative(AssociativeOperation {
            operator: operator as i32,
        })
    }

    fn binary(operator: binary_operation::Operator) -> WireInstruction {
        WireInstruction::Binary(BinaryOperation {
            operator: operator as i32,
        })
    }

    fn parse_error(value: v1::Function) -> ParseError {
        match Function::try_from(value) {
            Ok(_) => panic!("malformed expression unexpectedly parsed"),
            Err(error) => error,
        }
    }

    #[test]
    fn rpn_expression_preserves_instruction_order_and_roundtrips() {
        // ((abs(x1) min signum(x2)) / 2)^-3
        let original = expression(vec![
            WireInstruction::Linear(v1::Linear {
                terms: vec![v1::linear::Term {
                    id: 1,
                    coefficient: 1.0,
                }],
                constant: 0.0,
            }),
            unary(unary_operation::Operator::Abs),
            WireInstruction::Linear(v1::Linear {
                terms: vec![v1::linear::Term {
                    id: 2,
                    coefficient: 1.0,
                }],
                constant: 0.0,
            }),
            unary(unary_operation::Operator::Signum),
            associative(associative_operation::Operator::Min),
            WireInstruction::Constant(2.0),
            binary(binary_operation::Operator::Div),
            powi(Some(-3)),
        ]);

        let parsed = Function::try_from(original.clone()).unwrap();
        assert!(matches!(&parsed, Function::Expression(_)));
        assert_eq!(v1::Function::from(parsed.clone()), original);

        let bytes = parsed.to_bytes();
        assert_eq!(Function::from_bytes(&bytes).unwrap(), parsed);
    }

    #[test]
    fn single_atom_expression_normalizes_to_compact_function() {
        let parsed = Function::try_from(expression(vec![WireInstruction::Constant(2.0)])).unwrap();
        assert!(matches!(&parsed, Function::Constant(_)));
        assert!(matches!(
            v1::Function::from(parsed).function,
            Some(v1::function::Function::Constant(2.0))
        ));
    }

    #[test]
    fn expression_rejects_stack_underflow_and_invalid_final_height() {
        insta::assert_snapshot!(parse_error(expression(vec![])), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.Function[expression]
          └─ommx.v1.Function.Expression[instructions]
        expression program must leave exactly one stack value, found 0
        "###);

        insta::assert_snapshot!(
            parse_error(expression(vec![unary(unary_operation::Operator::Abs)])),
            @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.Function[expression]
          └─ommx.v1.Function.Expression[instructions]
        expression instruction 0 requires 1 stack values, found 0
        "###
        );

        insta::assert_snapshot!(
            parse_error(expression(vec![
                WireInstruction::Constant(1.0),
                associative(associative_operation::Operator::Add),
            ])),
            @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.Function[expression]
          └─ommx.v1.Function.Expression[instructions]
        expression instruction 1 requires 2 stack values, found 1
        "###
        );

        insta::assert_snapshot!(
            parse_error(expression(vec![
                WireInstruction::Constant(1.0),
                WireInstruction::Constant(2.0),
            ])),
            @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.Function[expression]
          └─ommx.v1.Function.Expression[instructions]
        expression program must leave exactly one stack value, found 2
        "###
        );
    }

    #[test]
    fn expression_rejects_missing_instruction() {
        let malformed = v1::Function {
            function: Some(v1::function::Function::Expression(
                v1::function::Expression {
                    instructions: vec![v1::function::expression::Instruction { instruction: None }],
                },
            )),
        };
        insta::assert_snapshot!(parse_error(malformed), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.Function[expression]
          └─ommx.v1.Function.Expression[instructions]
        Field instruction in ommx.v1.Function.Expression.Instruction is missing.
        "###);
    }

    #[test]
    fn expression_rejects_unknown_and_unspecified_operators() {
        let unary_error = |value| {
            parse_error(expression(vec![
                WireInstruction::Constant(1.0),
                WireInstruction::Unary(UnaryOperation {
                    operator: value,
                    integer_exponent: None,
                }),
            ]))
        };
        insta::assert_snapshot!(unary_error(0), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.Function[expression]
          └─ommx.v1.Function.Expression[instructions]
            └─ommx.v1.Function.Expression.Instruction[unary]
              └─ommx.v1.Function.Expression.Instruction.UnaryOperation[operator]
        Unknown or unsupported enum value 0 for ommx.v1.Function.Expression.Instruction.UnaryOperation.Operator. This may be due to an unspecified value or a newer version of the protocol.
        "###);
        insta::assert_snapshot!(unary_error(99), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.Function[expression]
          └─ommx.v1.Function.Expression[instructions]
            └─ommx.v1.Function.Expression.Instruction[unary]
              └─ommx.v1.Function.Expression.Instruction.UnaryOperation[operator]
        Unknown or unsupported enum value 99 for ommx.v1.Function.Expression.Instruction.UnaryOperation.Operator. This may be due to an unspecified value or a newer version of the protocol.
        "###);

        let associative_error = |value| {
            parse_error(expression(vec![
                WireInstruction::Constant(1.0),
                WireInstruction::Constant(2.0),
                WireInstruction::Associative(AssociativeOperation { operator: value }),
            ]))
        };
        insta::assert_snapshot!(associative_error(0), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.Function[expression]
          └─ommx.v1.Function.Expression[instructions]
            └─ommx.v1.Function.Expression.Instruction[associative]
              └─ommx.v1.Function.Expression.Instruction.AssociativeOperation[operator]
        Unknown or unsupported enum value 0 for ommx.v1.Function.Expression.Instruction.AssociativeOperation.Operator. This may be due to an unspecified value or a newer version of the protocol.
        "###);
        insta::assert_snapshot!(associative_error(99), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.Function[expression]
          └─ommx.v1.Function.Expression[instructions]
            └─ommx.v1.Function.Expression.Instruction[associative]
              └─ommx.v1.Function.Expression.Instruction.AssociativeOperation[operator]
        Unknown or unsupported enum value 99 for ommx.v1.Function.Expression.Instruction.AssociativeOperation.Operator. This may be due to an unspecified value or a newer version of the protocol.
        "###);

        let binary_error = |value| {
            parse_error(expression(vec![
                WireInstruction::Constant(1.0),
                WireInstruction::Constant(2.0),
                WireInstruction::Binary(BinaryOperation { operator: value }),
            ]))
        };
        insta::assert_snapshot!(binary_error(0), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.Function[expression]
          └─ommx.v1.Function.Expression[instructions]
            └─ommx.v1.Function.Expression.Instruction[binary]
              └─ommx.v1.Function.Expression.Instruction.BinaryOperation[operator]
        Unknown or unsupported enum value 0 for ommx.v1.Function.Expression.Instruction.BinaryOperation.Operator. This may be due to an unspecified value or a newer version of the protocol.
        "###);
        insta::assert_snapshot!(binary_error(99), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.Function[expression]
          └─ommx.v1.Function.Expression[instructions]
            └─ommx.v1.Function.Expression.Instruction[binary]
              └─ommx.v1.Function.Expression.Instruction.BinaryOperation[operator]
        Unknown or unsupported enum value 99 for ommx.v1.Function.Expression.Instruction.BinaryOperation.Operator. This may be due to an unspecified value or a newer version of the protocol.
        "###);
    }

    #[test]
    fn powi_requires_exponent_and_other_unary_operators_reject_it() {
        insta::assert_snapshot!(
            parse_error(expression(vec![WireInstruction::Constant(2.0), powi(None)])),
            @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.Function[expression]
          └─ommx.v1.Function.Expression[instructions]
            └─ommx.v1.Function.Expression.Instruction[unary]
        Field integer_exponent in ommx.v1.Function.Expression.Instruction.UnaryOperation is missing.
        "###
        );

        let malformed = expression(vec![
            WireInstruction::Constant(2.0),
            WireInstruction::Unary(UnaryOperation {
                operator: unary_operation::Operator::Abs as i32,
                integer_exponent: Some(2),
            }),
        ]);
        insta::assert_snapshot!(parse_error(malformed), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.Function[expression]
          └─ommx.v1.Function.Expression[instructions]
            └─ommx.v1.Function.Expression.Instruction[unary]
              └─ommx.v1.Function.Expression.Instruction.UnaryOperation[integer_exponent]
        integer_exponent is only valid for unary POWI instructions
        "###);
    }

    #[test]
    fn powi_exponent_is_exact_and_roundtrips() {
        for exponent in [i32::MIN, -2, 0, 1, 2, i32::MAX] {
            let original = expression(vec![WireInstruction::Constant(2.0), powi(Some(exponent))]);
            let parsed = Function::try_from(original.clone()).unwrap();
            assert_eq!(v1::Function::from(parsed), original);
        }
    }

    #[test]
    fn deeply_nested_expression_roundtrips_without_recursive_messages() {
        let mut instructions = vec![WireInstruction::Constant(2.0)];
        instructions.extend((0..4096).map(|_| unary(unary_operation::Operator::Abs)));
        let original = expression(instructions);
        let bytes = original.encode_to_vec();
        let parsed = Function::from_bytes(&bytes).unwrap();
        let roundtrip = v1::Function::decode(parsed.to_bytes().as_slice()).unwrap();
        assert_eq!(roundtrip, original);
    }

    proptest! {
        #[test]
        fn function_roundtrip(original in Function::arbitrary()) {
            let v1_function = v1::Function::from(original.clone());
            let parsed = Function::try_from(v1_function).unwrap();
            prop_assert_eq!(original, parsed);
        }

        #[test]
        fn polynomial_strategy_respects_parameters(
            (parameters, function) in PolynomialParameters::arbitrary().prop_flat_map(|parameters| {
                Function::arbitrary_with(FunctionParameters::polynomial_only(parameters))
                    .prop_map(move |function| (parameters, function))
            }),
        ) {
            prop_assert_eq!(function.num_terms(), Some(parameters.num_terms()));
            prop_assert!(function.degree().is_some_and(|degree| degree <= parameters.max_degree()));
        }
    }
}
