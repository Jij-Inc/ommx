//! Python SDK-owned receivers for bridge protocol v0.
//!
//! Independently built extension modules pass protobuf bytes to these
//! binding-private functions so the extension that owns the canonical OMMX
//! Python classes also reconstructs them. The versioned endpoint names identify
//! the exact argument and payload interpretation without protocol negotiation.

use crate::{Constraint, DecisionVariable, Function};
use ommx::{Message as _, Parse as _};
use pyo3::{prelude::*, types::PyBytes};

#[pyfunction]
fn _pyo3_bridge_v0_function_from_bytes(bytes: &Bound<'_, PyBytes>) -> anyhow::Result<Function> {
    Ok(Function(ommx::Function::from_bytes(bytes.as_bytes())?))
}

#[pyfunction]
fn _pyo3_bridge_v0_constraint_from_bytes(
    constraint: &Bound<'_, PyBytes>,
    context: &Bound<'_, PyBytes>,
) -> anyhow::Result<Constraint> {
    let constraint = ommx::v2::RegularConstraint::decode(constraint.as_bytes())?.parse(&())?;
    let context = ommx::v2::ConstraintContext::decode(context.as_bytes())?.parse(&())?;
    Ok(Constraint::from_parts(constraint, context))
}

#[pyfunction]
fn _pyo3_bridge_v0_decision_variable_from_bytes(
    id: u64,
    decision_variable: &Bound<'_, PyBytes>,
    label: &Bound<'_, PyBytes>,
) -> anyhow::Result<DecisionVariable> {
    let id = ommx::VariableID::from(id);
    let decision_variable =
        ommx::v2::DecisionVariable::decode(decision_variable.as_bytes())?.parse(&id)?;
    let label = ommx::v2::ModelingLabel::decode(label.as_bytes())?.into();
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
