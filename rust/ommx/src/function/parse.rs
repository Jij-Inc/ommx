use super::*;
use crate::{
    parse::{Parse, ParseError, RawParseError},
    v1, CoefficientError,
};

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
        let message = "ommx.v1.Function";
        use v1::function::Function::*;
        match self.function.ok_or(RawParseError::UnsupportedV1Function)? {
            Constant(c) => match c.try_into() {
                Ok(c) => Ok(Function::Constant(c)),
                Err(CoefficientError::Zero) => Ok(Function::Zero),
                Err(c) => Err(RawParseError::from(c).context(message, "constant")),
            },
            Linear(l) => Ok(Function::Linear(l.parse_as(&(), message, "linear")?)),
            Quadratic(q) => Ok(Function::Quadratic(q.parse_as(
                &(),
                message,
                "quadratic",
            )?)),
            Polynomial(p) => Ok(Function::Polynomial(p.parse_as(
                &(),
                message,
                "polynomial",
            )?)),
            Unary(operation) => (*operation).parse_as(&(), message, "unary"),
            Nary(operation) => operation.parse_as(&(), message, "nary"),
            Binary(operation) => (*operation).parse_as(&(), message, "binary"),
        }
    }
}

impl Parse for v1::function::UnaryOperation {
    type Output = Function;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.Function.UnaryOperation";
        let operator = match v1::function::unary_operation::Operator::try_from(self.operator) {
            Ok(v1::function::unary_operation::Operator::Neg) => UnaryOperator::Neg,
            Ok(v1::function::unary_operation::Operator::Abs) => UnaryOperator::Abs,
            Ok(v1::function::unary_operation::Operator::Signum) => UnaryOperator::Signum,
            Ok(v1::function::unary_operation::Operator::Unspecified) | Err(_) => {
                return Err(RawParseError::UnknownEnumValue {
                    enum_name: "ommx.v1.Function.UnaryOperation.Operator",
                    value: self.operator,
                }
                .context(message, "operator"));
            }
        };
        let operand = *self.operand.ok_or(RawParseError::MissingField {
            message,
            field: "operand",
        })?;
        let operand = operand.parse_as(&(), message, "operand")?;
        Ok(Function::unary_operation(operator, operand))
    }
}

impl Parse for v1::function::NaryOperation {
    type Output = Function;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.Function.NaryOperation";
        let operator = match v1::function::nary_operation::Operator::try_from(self.operator) {
            Ok(v1::function::nary_operation::Operator::Add) => NaryOperator::Add,
            Ok(v1::function::nary_operation::Operator::Mul) => NaryOperator::Mul,
            Ok(v1::function::nary_operation::Operator::Min) => NaryOperator::Min,
            Ok(v1::function::nary_operation::Operator::Max) => NaryOperator::Max,
            Ok(v1::function::nary_operation::Operator::Unspecified) | Err(_) => {
                return Err(RawParseError::UnknownEnumValue {
                    enum_name: "ommx.v1.Function.NaryOperation.Operator",
                    value: self.operator,
                }
                .context(message, "operator"));
            }
        };
        if self.operands.len() < 2 {
            return Err(RawParseError::InvalidFunction(format!(
                "NaryOperation requires at least two operands, found {}",
                self.operands.len()
            ))
            .context(message, "operands"));
        }
        let operands = self
            .operands
            .into_iter()
            .map(|operand| operand.parse_as(&(), message, "operands"))
            .collect::<Result<Vec<_>, _>>()?;
        // Preserve the explicit wire tree. Normalizing an untrusted n-ary
        // polynomial product here can expand O(n) input into 2^n terms.
        Ok(Function::nary_expression(operator, operands))
    }
}

impl Parse for v1::function::BinaryOperation {
    type Output = Function;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.Function.BinaryOperation";
        let operator = match v1::function::binary_operation::Operator::try_from(self.operator) {
            Ok(v1::function::binary_operation::Operator::Div) => BinaryOperator::Div,
            Ok(v1::function::binary_operation::Operator::Pow) => BinaryOperator::Pow,
            Ok(v1::function::binary_operation::Operator::Unspecified) | Err(_) => {
                return Err(RawParseError::UnknownEnumValue {
                    enum_name: "ommx.v1.Function.BinaryOperation.Operator",
                    value: self.operator,
                }
                .context(message, "operator"));
            }
        };
        let lhs = *self.lhs.ok_or(RawParseError::MissingField {
            message,
            field: "lhs",
        })?;
        let rhs = *self.rhs.ok_or(RawParseError::MissingField {
            message,
            field: "rhs",
        })?;
        let lhs = lhs.parse_as(&(), message, "lhs")?;
        let rhs = rhs.parse_as(&(), message, "rhs")?;
        Ok(Function::binary_expression(operator, lhs, rhs))
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
        use v1::function::Function::*;
        let function = match value {
            Function::Zero => Constant(0.0),
            Function::Constant(c) => Constant(c.into()),
            Function::Linear(l) => Linear(l.into()),
            Function::Quadratic(q) => Quadratic(q.into()),
            Function::Polynomial(p) => Polynomial(p.into()),
            Function::Unary(operation) => {
                let (operator, operand) = operation.into_parts();
                Unary(Box::new(v1::function::UnaryOperation {
                    operator: match operator {
                        UnaryOperator::Neg => v1::function::unary_operation::Operator::Neg as i32,
                        UnaryOperator::Abs => v1::function::unary_operation::Operator::Abs as i32,
                        UnaryOperator::Signum => {
                            v1::function::unary_operation::Operator::Signum as i32
                        }
                    },
                    operand: Some(Box::new(operand.into())),
                }))
            }
            Function::Nary(operation) => {
                let (operator, operands) = operation.into_parts();
                Nary(v1::function::NaryOperation {
                    operator: match operator {
                        NaryOperator::Add => v1::function::nary_operation::Operator::Add as i32,
                        NaryOperator::Mul => v1::function::nary_operation::Operator::Mul as i32,
                        NaryOperator::Min => v1::function::nary_operation::Operator::Min as i32,
                        NaryOperator::Max => v1::function::nary_operation::Operator::Max as i32,
                    },
                    operands: operands.into_iter().map(Into::into).collect(),
                })
            }
            Function::Binary(operation) => {
                let (operator, lhs, rhs) = operation.into_parts();
                Binary(Box::new(v1::function::BinaryOperation {
                    operator: match operator {
                        BinaryOperator::Div => v1::function::binary_operation::Operator::Div as i32,
                        BinaryOperator::Pow => v1::function::binary_operation::Operator::Pow as i32,
                    },
                    lhs: Some(Box::new(lhs.into())),
                    rhs: Some(Box::new(rhs.into())),
                }))
            }
        };
        Self {
            function: Some(function),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ATol, Evaluate, FunctionEvaluationError, Message, PolynomialParameters};
    use proptest::prelude::*;

    fn constant(value: f64) -> v1::Function {
        v1::Function {
            function: Some(v1::function::Function::Constant(value)),
        }
    }

    fn variable(id: u64) -> v1::Function {
        scaled_variable(id, 1.0, 0.0)
    }

    fn scaled_variable(id: u64, coefficient: f64, constant: f64) -> v1::Function {
        v1::Function {
            function: Some(v1::function::Function::Linear(v1::Linear {
                terms: vec![v1::linear::Term { id, coefficient }],
                constant,
            })),
        }
    }

    fn unary(operator: i32, operand: Option<v1::Function>) -> v1::Function {
        v1::Function {
            function: Some(v1::function::Function::Unary(Box::new(
                v1::function::UnaryOperation {
                    operator,
                    operand: operand.map(Box::new),
                },
            ))),
        }
    }

    fn nary(operator: i32, operands: Vec<v1::Function>) -> v1::Function {
        v1::Function {
            function: Some(v1::function::Function::Nary(v1::function::NaryOperation {
                operator,
                operands,
            })),
        }
    }

    fn binary(operator: i32, lhs: Option<v1::Function>, rhs: Option<v1::Function>) -> v1::Function {
        v1::Function {
            function: Some(v1::function::Function::Binary(Box::new(
                v1::function::BinaryOperation {
                    operator,
                    lhs: lhs.map(Box::new),
                    rhs: rhs.map(Box::new),
                },
            ))),
        }
    }

    fn assert_unknown_operator(
        result: Result<Function, ParseError>,
        expected_enum_name: &'static str,
        expected_value: i32,
    ) {
        let err = match result {
            Ok(_) => panic!("malformed operator unexpectedly parsed"),
            Err(err) => err,
        };
        assert!(matches!(
            err.error,
            RawParseError::UnknownEnumValue { enum_name, value }
                if enum_name == expected_enum_name && value == expected_value
        ));
    }

    #[test]
    fn test_parse_rejects_unknown_and_unspecified_operators() {
        for value in [0, 99] {
            assert_unknown_operator(
                Function::try_from(unary(value, Some(constant(1.0)))),
                "ommx.v1.Function.UnaryOperation.Operator",
                value,
            );
            assert_unknown_operator(
                Function::try_from(nary(value, vec![constant(1.0), constant(2.0)])),
                "ommx.v1.Function.NaryOperation.Operator",
                value,
            );
            assert_unknown_operator(
                Function::try_from(binary(value, Some(constant(1.0)), Some(constant(2.0)))),
                "ommx.v1.Function.BinaryOperation.Operator",
                value,
            );
        }
    }

    #[test]
    fn test_parse_rejects_missing_unary_operand() {
        let result = Function::try_from(unary(
            v1::function::unary_operation::Operator::Abs as i32,
            None,
        ));
        let err = match result {
            Ok(_) => panic!("unary operation without an operand unexpectedly parsed"),
            Err(err) => err,
        };
        assert!(matches!(
            err.error,
            RawParseError::MissingField {
                message: "ommx.v1.Function.UnaryOperation",
                field: "operand"
            }
        ));
    }

    #[test]
    fn test_parse_rejects_nary_operation_with_fewer_than_two_operands() {
        let operator = v1::function::nary_operation::Operator::Min as i32;
        for operands in [vec![], vec![constant(1.0)]] {
            let result = Function::try_from(nary(operator, operands));
            let err = match result {
                Ok(_) => panic!("nary operation with invalid arity unexpectedly parsed"),
                Err(err) => err,
            };
            assert!(matches!(
                err.error,
                RawParseError::InvalidFunction(ref message)
                    if message.contains("requires at least two operands")
            ));
        }
    }

    #[test]
    fn test_parse_rejects_missing_binary_operands() {
        let operator = v1::function::binary_operation::Operator::Div as i32;
        for (lhs, rhs, expected_field) in [
            (None, Some(constant(2.0)), "lhs"),
            (Some(constant(1.0)), None, "rhs"),
        ] {
            let result = Function::try_from(binary(operator, lhs, rhs));
            let err = match result {
                Ok(_) => panic!("binary operation with a missing operand unexpectedly parsed"),
                Err(err) => err,
            };
            assert!(matches!(
                err.error,
                RawParseError::MissingField {
                    message: "ommx.v1.Function.BinaryOperation",
                    field
                } if field == expected_field
            ));
        }
    }

    #[test]
    fn test_recursive_function_roundtrip() {
        use v1::function::{
            binary_operation::Operator as Binary, nary_operation::Operator as Nary,
            unary_operation::Operator as Unary,
        };

        let numerator = nary(
            Nary::Add as i32,
            vec![
                nary(
                    Nary::Min as i32,
                    vec![
                        unary(Unary::Abs as i32, Some(variable(1))),
                        unary(Unary::Signum as i32, Some(variable(2))),
                    ],
                ),
                unary(
                    Unary::Neg as i32,
                    Some(unary(Unary::Abs as i32, Some(variable(3)))),
                ),
            ],
        );
        let denominator = nary(
            Nary::Mul as i32,
            vec![
                nary(
                    Nary::Max as i32,
                    vec![
                        unary(Unary::Signum as i32, Some(variable(4))),
                        constant(1.0),
                    ],
                ),
                binary(
                    Binary::Pow as i32,
                    Some(variable(5)),
                    Some(unary(Unary::Abs as i32, Some(variable(6)))),
                ),
            ],
        );
        let original = Function::try_from(binary(
            Binary::Div as i32,
            Some(numerator),
            Some(denominator),
        ))
        .unwrap();

        let bytes = original.to_bytes().unwrap();
        let encoded = v1::Function::decode(bytes.as_slice()).unwrap();
        assert!(matches!(
            encoded.function,
            Some(v1::function::Function::Binary(_))
        ));
        let parsed = Function::from_bytes(&bytes).unwrap();
        assert!(original == parsed);
    }

    #[test]
    fn parse_preserves_ordered_nary_evaluation_and_avoids_polynomial_expansion() {
        let operator = v1::function::nary_operation::Operator::Mul as i32;
        let expression = Function::try_from(nary(
            operator,
            vec![
                scaled_variable(1, 1e150, 0.0),
                scaled_variable(1, 1e150, 0.0),
            ],
        ))
        .unwrap();

        let Function::Nary(operation) = &expression else {
            panic!("wire n-ary expression must retain its explicit tree");
        };
        assert_eq!(operation.operator(), NaryOperator::Mul);
        let state = v1::State::from_iter([(1, 1e-200)]);
        let operand: f64 = 1e150 * 1e-200;
        assert_eq!(
            expression
                .evaluate(&state, ATol::default())
                .unwrap()
                .to_bits(),
            (operand * operand).to_bits(),
        );

        // This family has 2^n compact polynomial terms if multiplied out.
        // Parsing must remain linear in the number of wire operands.
        let operands = (1..=20).map(|id| scaled_variable(id, 1.0, 1.0)).collect();
        let expression = Function::try_from(nary(operator, operands)).unwrap();
        let Function::Nary(operation) = expression else {
            panic!("wire n-ary expression must not be eagerly expanded");
        };
        assert_eq!(operation.operands().len(), 20);
    }

    #[test]
    fn parse_preserves_binary_division_domain_and_evaluation_order() {
        let expression = Function::try_from(binary(
            v1::function::binary_operation::Operator::Div as i32,
            Some(scaled_variable(1, f64::MAX, 0.0)),
            Some(constant(2.0)),
        ))
        .unwrap();
        assert!(matches!(expression, Function::Binary(_)));

        let error = expression
            .evaluate(&v1::State::from_iter([(1, 2.0)]), ATol::default())
            .unwrap_err();
        assert!(matches!(
            error.downcast_ref::<FunctionEvaluationError>(),
            Some(FunctionEvaluationError::NonFiniteResult { .. })
        ));
    }

    proptest! {
        #[test]
        fn test_function(
            (p, function) in PolynomialParameters::arbitrary()
                .prop_flat_map(|p| {
                    Function::arbitrary_with(p)
                        .prop_map(move |function| (p, function))
                }),
        ) {
            prop_assert_eq!(function.num_terms(), Some(p.num_terms()));
            prop_assert!(function.degree().is_some_and(|degree| degree <= p.max_degree()));
            for (monomial, _) in function
                .iter()
                .expect("PolynomialParameters only generate polynomial functions")
            {
                for id in monomial.iter() {
                    prop_assert!(*id <= p.max_id());
                }
            }
        }

        /// Function -> v1::Function -> Function
        #[test]
        fn test_function_roundtrip(original in Function::arbitrary()) {
            let v1_function = v1::Function::from(original.clone());
            let parsed = Function::try_from(v1_function).unwrap();
            prop_assert_eq!(original, parsed);
        }
    }
}
