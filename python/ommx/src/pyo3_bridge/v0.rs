//! Python SDK-owned receivers for bridge protocol v0.
//!
//! Independently built extension modules pass protobuf bytes to these
//! binding-private functions so the extension that owns the canonical OMMX
//! Python classes also reconstructs them. The versioned endpoint names identify
//! the exact argument and payload interpretation without protocol negotiation.

use crate::{Constraint, DecisionVariable, Function};
use ommx::Parse as _;
use pyo3::{exceptions::PyRuntimeError, prelude::*, types::PyBytes};

/// A malformed v0 payload indicates a broken or incompatible internal bridge,
/// not invalid input to a public Python decoder.
#[derive(Debug)]
struct BridgeProtocolError(ommx::Error);

type BridgeProtocolResult<T> = std::result::Result<T, BridgeProtocolError>;

impl From<ommx::Error> for BridgeProtocolError {
    fn from(error: ommx::Error) -> Self {
        Self(error)
    }
}

impl From<ommx::ParseError> for BridgeProtocolError {
    fn from(error: ommx::ParseError) -> Self {
        Self(error.into())
    }
}

impl From<BridgeProtocolError> for PyErr {
    fn from(BridgeProtocolError(error): BridgeProtocolError) -> Self {
        PyRuntimeError::new_err(format!("invalid OMMX PyO3 bridge v0 payload: {error:#}"))
    }
}

fn decode<M>(bytes: &[u8], root: &'static str) -> BridgeProtocolResult<M>
where
    M: ommx::Message + Default,
{
    let message = M::decode(bytes)
        .map_err(|error| ommx::RawParseError::from(error).context(root, "bytes"))?;
    Ok(message)
}

#[pyfunction]
fn _pyo3_bridge_v0_function_from_bytes(
    bytes: &Bound<'_, PyBytes>,
) -> BridgeProtocolResult<Function> {
    Ok(Function(ommx::Function::from_bytes(bytes.as_bytes())?))
}

#[pyfunction]
fn _pyo3_bridge_v0_constraint_from_bytes(
    constraint: &Bound<'_, PyBytes>,
    context: &Bound<'_, PyBytes>,
) -> BridgeProtocolResult<Constraint> {
    let constraint =
        decode::<ommx::v2::RegularConstraint>(constraint.as_bytes(), "ommx.v2.RegularConstraint")?
            .parse(&())?;
    let context =
        decode::<ommx::v2::ConstraintContext>(context.as_bytes(), "ommx.v2.ConstraintContext")?
            .parse(&())?;
    Ok(Constraint::from_parts(constraint, context))
}

#[pyfunction]
fn _pyo3_bridge_v0_decision_variable_from_bytes(
    id: u64,
    decision_variable: &Bound<'_, PyBytes>,
    label: &Bound<'_, PyBytes>,
) -> BridgeProtocolResult<DecisionVariable> {
    let id = ommx::VariableID::from(id);
    let decision_variable = decode::<ommx::v2::DecisionVariable>(
        decision_variable.as_bytes(),
        "ommx.v2.DecisionVariable",
    )?
    .parse(&id)?;
    let label =
        decode::<ommx::v2::ModelingLabel>(label.as_bytes(), "ommx.v2.ModelingLabel")?.into();
    Ok(DecisionVariable::from_parts(id, decision_variable, label))
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        _pyo3_bridge_v0_function_from_bytes,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        _pyo3_bridge_v0_constraint_from_bytes,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        _pyo3_bridge_v0_decision_variable_from_bytes,
        module
    )?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_payload_is_a_bridge_runtime_error() {
        Python::initialize();
        Python::attach(|py| {
            let bytes = PyBytes::new(py, b"\xff");
            let error = _pyo3_bridge_v0_function_from_bytes(&bytes)
                .err()
                .expect("invalid bridge payload must fail");
            let error: PyErr = error.into();

            assert!(error.is_instance_of::<PyRuntimeError>(py));
            assert!(
                error
                    .to_string()
                    .contains("invalid OMMX PyO3 bridge v0 payload"),
                "{error}"
            );
        });
    }
}
