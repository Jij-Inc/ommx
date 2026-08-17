use super::*;
use crate::{message_io, parse::RawParseError, v1, Message, Parse};
use anyhow::Result;

impl Function {
    /// Maximum number of nested expression-operation nodes accepted by the
    /// protobuf byte APIs.
    ///
    /// Prost limits protobuf message recursion to 100 levels. Each expression
    /// operation adds at least two protobuf message levels (`Function` plus its
    /// operation payload), and an enclosing Instance adds several more. Keeping
    /// the expression tree at 32 operation nodes leaves enough budget for every
    /// supported v1/v2 root container.
    pub(crate) const MAX_WIRE_EXPRESSION_DEPTH: usize = 32;

    /// Serialize this function as `ommx.v1.Function` protobuf bytes.
    ///
    /// Returns an error when the recursive expression exceeds the depth that
    /// can safely round-trip through supported protobuf root containers.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        self.ensure_wire_expression_depth()?;
        let v1_function = v1::Function::from(self.clone());
        Ok(v1_function.encode_to_vec())
    }

    /// Deserialize an `ommx.v1.Function` from protobuf bytes.
    ///
    /// Returns a [`crate::ParseError`] in the error chain when the protobuf is
    /// malformed, semantically invalid, or exceeds the expression depth that
    /// the managed byte APIs can safely round-trip.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let inner = message_io::decode::<v1::Function>(bytes, "ommx.v1.Function")?;
        let function = Parse::parse(inner, &())?;
        function.ensure_wire_expression_depth().map_err(|error| {
            RawParseError::InvalidFunction(format!("{error:#}"))
                .context("ommx.v1.Function", "bytes")
        })?;
        Ok(function)
    }

    /// Validate the recursive depth without recursively walking the Rust call
    /// stack. This check must run before cloning or converting a Function for
    /// protobuf serialization, since those operations are themselves
    /// recursive for expression trees.
    pub(crate) fn ensure_wire_expression_depth(&self) -> Result<()> {
        let mut pending = vec![(self, 0usize)];
        while let Some((function, depth)) = pending.pop() {
            if depth > Self::MAX_WIRE_EXPRESSION_DEPTH {
                crate::bail!(
                    {
                        expression_depth = depth,
                        max_expression_depth = Self::MAX_WIRE_EXPRESSION_DEPTH
                    },
                    "Function expression depth exceeds the protobuf serialization limit of {}",
                    Self::MAX_WIRE_EXPRESSION_DEPTH,
                );
            }
            let child_depth = depth + 1;
            match function {
                Function::Unary(operation) => {
                    pending.push((operation.operand(), child_depth));
                }
                Function::Nary(operation) => {
                    pending.extend(
                        operation
                            .operands()
                            .iter()
                            .map(|operand| (operand, child_depth)),
                    );
                }
                Function::Binary(operation) => {
                    pending.push((operation.lhs(), child_depth));
                    pending.push((operation.rhs(), child_depth));
                }
                Function::Zero
                | Function::Constant(_)
                | Function::Linear(_)
                | Function::Quadratic(_)
                | Function::Polynomial(_) => {}
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linear;

    fn function_at_depth(depth: usize) -> Function {
        (0..depth).fold(Function::from(linear!(1)), |function, level| {
            if level % 2 == 0 {
                function.abs()
            } else {
                function.signum()
            }
        })
    }

    fn raw_powi_identity_at_depth(depth: usize) -> v1::Function {
        (0..depth).fold(
            v1::Function::from(Function::from(linear!(1))),
            |operand, _| v1::Function {
                function: Some(v1::function::Function::Unary(Box::new(
                    v1::function::UnaryOperation {
                        operator: v1::function::unary_operation::Operator::Powi as i32,
                        operand: Some(Box::new(operand)),
                        integer_exponent: Some(1),
                    },
                ))),
            },
        )
    }

    #[test]
    fn protobuf_bytes_enforce_expression_depth_boundary() {
        let boundary = function_at_depth(Function::MAX_WIRE_EXPRESSION_DEPTH);
        let bytes = boundary.to_bytes().unwrap();
        assert_eq!(Function::from_bytes(&bytes).unwrap(), boundary);

        let too_deep = function_at_depth(Function::MAX_WIRE_EXPRESSION_DEPTH + 1);
        let error = too_deep.to_bytes().unwrap_err();
        assert!(
            error
                .to_string()
                .contains("protobuf serialization limit of 32"),
            "unexpected error: {error:#}",
        );

        // Raw protobuf values are an intentional low-level escape hatch, but
        // the managed decoder must not admit a Function that its encoder
        // immediately rejects.
        let raw_bytes = v1::Function::from(too_deep).encode_to_vec();
        let error = Function::from_bytes(&raw_bytes).unwrap_err();
        assert!(error.downcast_ref::<crate::ParseError>().is_some());
        assert!(
            format!("{error:#}").contains("protobuf serialization limit of 32"),
            "unexpected error: {error:#}",
        );

        // Identity powers are normalized by the public constructor, but a
        // wire payload must be depth-checked before any such simplification.
        let raw_bytes =
            raw_powi_identity_at_depth(Function::MAX_WIRE_EXPRESSION_DEPTH + 1).encode_to_vec();
        let error = Function::from_bytes(&raw_bytes).unwrap_err();
        assert!(error.downcast_ref::<crate::ParseError>().is_some());
        assert!(
            format!("{error:#}").contains("protobuf serialization limit of 32"),
            "unexpected error: {error:#}",
        );
    }
}
