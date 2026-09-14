//! Configure the SDK-owned Python constructors for the bridge's receivers.

use crate::{Constraint, DecisionVariable, Function};
use ommx_pyo3_bridge::{ReceiverConfig, TransferProtocolId};
use pyo3::{prelude::*, IntoPyObjectExt};

// SDK initialization calls this after registering the canonical classes.
pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    ReceiverConfig {
        protocols: vec![
            TransferProtocolId::ProtobufV1,
            TransferProtocolId::ProtobufV2,
        ],
        legacy_v0: true,
        function: |py, value| Function(value).into_py_any(py),
        constraint: |py, value, context| Constraint::from_parts(value, context).into_py_any(py),
        decision_variable: |py, id, value, label| {
            DecisionVariable::from_parts(id, value, label).into_py_any(py)
        },
    }
    .register(module)
}
