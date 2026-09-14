//! Versioned production receivers for `ommx-pyo3-bridge`.
//!
//! Protocol implementations stay binding-private and out of the generated
//! Python API. Registering a new protocol alongside an existing one allows
//! their exact endpoint and payload interpretations to coexist.

mod protobuf_v1;
mod v0;

use pyo3::prelude::*;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    v0::register(module)?;
    protobuf_v1::register(module)?;
    module.add_function(wrap_pyfunction!(_bridge_supported_protocols, module)?)?;
    Ok(())
}

#[pyfunction]
fn _bridge_supported_protocols() -> Vec<u32> {
    use ommx_pyo3_bridge::TransferProtocolId;
    vec![
        TransferProtocolId::ProtobufV1 as u32,
        TransferProtocolId::ProtobufV2 as u32,
    ]
}
