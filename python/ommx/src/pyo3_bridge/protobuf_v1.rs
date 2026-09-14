//! Legacy component receivers. Root receivers use the public versioned APIs.

use crate::{Constraint, DecisionVariable, Function};
use ommx::Parse;
use pyo3::{exceptions::PyRuntimeError, prelude::*, types::PyBytes};

#[derive(Debug)]
struct BridgePayloadError(ommx::Error);

type BridgeResult<T> = Result<T, BridgePayloadError>;

impl From<ommx::Error> for BridgePayloadError {
    fn from(error: ommx::Error) -> Self {
        Self(error)
    }
}

impl From<ommx::ParseError> for BridgePayloadError {
    fn from(error: ommx::ParseError) -> Self {
        Self(error.into())
    }
}

impl From<BridgePayloadError> for PyErr {
    fn from(BridgePayloadError(error): BridgePayloadError) -> Self {
        PyRuntimeError::new_err(format!("invalid OMMX ProtobufV1 bridge payload: {error:#}"))
    }
}

fn decode<M: ommx::Message + Default>(bytes: &[u8], root: &'static str) -> BridgeResult<M> {
    Ok(
        M::decode(bytes)
            .map_err(|error| ommx::RawParseError::from(error).context(root, "bytes"))?,
    )
}

#[pyfunction]
fn _bridge_protobuf_v1_function_from_bytes(bytes: &Bound<'_, PyBytes>) -> BridgeResult<Function> {
    Ok(Function(ommx::Function::from_bytes(bytes.as_bytes())?))
}

#[pyfunction]
fn _bridge_protobuf_v1_constraint_from_bytes(
    bytes: &Bound<'_, PyBytes>,
) -> BridgeResult<Constraint> {
    let message = decode::<ommx::v1::Constraint>(bytes.as_bytes(), "ommx.v1.Constraint")?;
    let (_, constraint, context) = message.parse(&())?;
    Ok(Constraint::from_parts(constraint, context))
}

#[pyfunction]
fn _bridge_protobuf_v1_decision_variable_from_bytes(
    bytes: &Bound<'_, PyBytes>,
) -> BridgeResult<DecisionVariable> {
    let message =
        decode::<ommx::v1::DecisionVariable>(bytes.as_bytes(), "ommx.v1.DecisionVariable")?;
    let parsed = message.parse(&())?;
    if parsed.fixed_value.is_some() {
        return Err(ommx::Error::msg(
            "a detached DecisionVariable cannot own a fixed value; transfer its Instance instead",
        )
        .into());
    }
    Ok(DecisionVariable::from_parts(
        parsed.id,
        parsed.variable,
        parsed.label,
    ))
}

pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        _bridge_protobuf_v1_function_from_bytes,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        _bridge_protobuf_v1_constraint_from_bytes,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        _bridge_protobuf_v1_decision_variable_from_bytes,
        module
    )?)?;
    Ok(())
}
