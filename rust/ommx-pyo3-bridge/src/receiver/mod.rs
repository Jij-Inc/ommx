//! Register protocol-specific receivers inside the installed Python SDK.

mod protobuf;
mod protobuf_v1;
mod protobuf_v2;

pub use protobuf_v1::ProtobufV1ReceiverConfig;
pub use protobuf_v2::ProtobufV2ReceiverConfig;

use crate::{protocol, TransferProtocolId};
use pyo3::{exceptions::PyImportError, prelude::*};

/// One complete protocol configuration in a receiver registration list.
///
/// Each variant's concrete configuration fixes its supported types and factory
/// signatures. This enum only collects heterogeneous configurations for
/// [`register_receivers`]; it imposes no common payload or factory input type.
/// Future representations may have different ownership requirements.
#[non_exhaustive]
pub enum ReceiverConfig {
    /// Receive the ProtobufV1 contract.
    ProtobufV1(ProtobufV1ReceiverConfig),
    /// Receive the ProtobufV2 contract.
    ProtobufV2(ProtobufV2ReceiverConfig),
}

impl From<ProtobufV1ReceiverConfig> for ReceiverConfig {
    fn from(config: ProtobufV1ReceiverConfig) -> Self {
        Self::ProtobufV1(config)
    }
}

impl From<ProtobufV2ReceiverConfig> for ReceiverConfig {
    fn from(config: ProtobufV2ReceiverConfig) -> Self {
        Self::ProtobufV2(config)
    }
}

impl ReceiverConfig {
    fn id(&self) -> TransferProtocolId {
        match self {
            Self::ProtobufV1(_) => TransferProtocolId::ProtobufV1,
            Self::ProtobufV2(_) => TransferProtocolId::ProtobufV2,
        }
    }

    fn bindings(self, py: Python<'_>) -> PyResult<Vec<Binding>> {
        match self {
            Self::ProtobufV1(config) => protobuf_v1::bindings(config, py),
            Self::ProtobufV2(config) => protobuf_v2::bindings(config, py),
        }
    }
}

type Binding = (&'static str, Py<PyAny>);

fn bind_methods(
    receiver: Bound<'_, PyAny>,
    methods: &[(&'static str, &'static str)],
) -> PyResult<Vec<Binding>> {
    methods
        .iter()
        .map(|&(endpoint, method)| Ok((endpoint, receiver.getattr(method)?.unbind())))
        .collect()
}

/// Register one list of protocol configurations on the SDK's extension module.
///
/// The bridge derives the supported-protocol declaration from the list, in
/// registration order, and publishes it after preparing all receivers. Every
/// type uses a bridge-owned private endpoint; public Python decoding methods
/// and class constructors are not consulted.
///
/// Duplicate protocols, an existing declaration, or occupied endpoint names
/// raise `ImportError` before any attributes are added. Supply the complete
/// list in one call per module. An empty list advertises no supported protocols.
/// Factories and their Python objects belong to this receiving extension; no
/// process-global receiver state or cross-extension Rust objects are used.
pub fn register_receivers(
    module: &Bound<'_, PyModule>,
    configs: impl IntoIterator<Item = ReceiverConfig>,
) -> PyResult<()> {
    let configs: Vec<_> = configs.into_iter().collect();
    let mut ids = Vec::new();
    for config in &configs {
        let id = config.id() as u32;
        if ids.contains(&id) {
            return Err(PyImportError::new_err(format!(
                "OMMX bridge receiver configuration repeats {:?}",
                config.id(),
            )));
        }
        ids.push(id);
    }
    if module.hasattr(protocol::SUPPORTED_PROTOCOLS)? {
        return Err(PyImportError::new_err(
            "OMMX bridge receivers are already registered",
        ));
    }

    let py = module.py();
    let mut bindings = Vec::new();
    for config in configs {
        bindings.extend(config.bindings(py)?);
    }
    let declaration = Py::new(py, ProtocolDeclaration { ids })?.into_bound(py);
    bindings.push((
        protocol::SUPPORTED_PROTOCOLS,
        declaration.getattr("supported_protocols")?.unbind(),
    ));
    for (name, _) in &bindings {
        if module.hasattr(*name)? {
            return Err(PyImportError::new_err(format!(
                "OMMX bridge receiver endpoint {name} already exists",
            )));
        }
    }
    // All Python allocations and contract checks complete before publication.
    // The final binding is the declaration, so support is advertised last.
    for (name, method) in bindings {
        module.add(name, method)?;
    }
    Ok(())
}

#[pyclass(frozen, module = "ommx._ommx_rust")]
struct ProtocolDeclaration {
    ids: Vec<u32>,
}

#[pymethods]
impl ProtocolDeclaration {
    fn supported_protocols(&self) -> Vec<u32> {
        self.ids.clone()
    }
}

#[cfg(test)]
mod tests;
