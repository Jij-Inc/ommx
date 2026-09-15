//! Protocol probes let callers choose their compilation path. Typed targets
//! retain the selected protocol and loaded SDK for subsequent transfers.

mod protobuf;

use pyo3::{
    exceptions::{PyImportError, PyRuntimeError},
    prelude::*,
};
use pyo3_stub_gen::PyStubType;
use std::marker::PhantomData;

/// Fixed transfer contracts, independent of SDK and protobuf version numbers.
/// IDs are never reused and their numeric order does not express preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
#[non_exhaustive]
pub enum TransferProtocolId {
    /// Legacy protobuf roots and complete, ID-bearing component messages.
    ProtobufV1 = 1,
    /// Normalized protobuf roots and detached component reconstruction.
    ProtobufV2 = 2,
}

mod sealed {
    pub trait Protocol {}
    pub trait Type<P> {}
}

/// A bridge-owned transfer contract, including representation and ownership.
/// New contracts get a new ID; codecs determine which values they can encode.
pub trait TransferProtocol: sealed::Protocol {
    /// Stable ID advertised by the Python receiver.
    const ID: TransferProtocolId;
    /// Human-readable name used in diagnostics.
    const NAME: &'static str;
}

/// Transfer through the OMMX v1 protobuf representations.
#[derive(Debug)]
pub struct ProtobufV1;
/// Transfer through normalized roots and the established detached receivers.
#[derive(Debug)]
pub struct ProtobufV2;

macro_rules! protocol_marker {
    ($name:ident) => {
        impl sealed::Protocol for $name {}
        impl TransferProtocol for $name {
            const ID: TransferProtocolId = TransferProtocolId::$name;
            const NAME: &'static str = stringify!($name);
        }
    };
}
protocol_marker!(ProtobufV1);
protocol_marker!(ProtobufV2);

/// A Python return type that can be reconstructed through protocol `P`.
///
/// Implementations describe a type/protocol pair, not a payload's features.
/// The receiver must validate the payload. The payload owns its resources until
/// import completes or fails.
pub trait TransferVia<P: TransferProtocol>: PyStubType + sealed::Type<P> + Sized {
    /// Exported representation. It need not be a byte buffer.
    type Payload;
    /// Canonical Python type name, for diagnostics.
    const PYTHON_NAME: &'static str;
    /// Resolve the reconstruction callable from the loaded Python SDK.
    fn receiver(module: &Bound<'_, PyModule>) -> PyResult<Py<PyAny>>;
    /// Reconstruct the canonical Python object without renegotiation.
    fn import(payload: Self::Payload, receiver: &Bound<'_, PyAny>) -> PyResult<Self>;
}

/// Convert a complete source value into the representation for `T` over `P`.
///
/// Multiple input types may produce the same Python type. In particular, v1
/// roots can be exported directly, preserving legacy ConstraintHints that the
/// Rust v3 domain model intentionally does not store.
///
/// A source must support the selected protocol at compile time:
///
/// ```compile_fail
/// use ommx_pyo3_bridge::{ProtobufV2, PyInstance, Target};
/// use pyo3::prelude::*;
/// fn wrong_format(py: Python<'_>, target: Target<ProtobufV2>) -> PyResult<PyInstance> {
///     target.transfer(py, ommx::v1::Instance::default())
/// }
/// ```
///
/// `Solution` and `SampleSet` require explicit v1 messages for ProtobufV1.
/// Their existing domain-to-v1 conversions are lossy and are not implicitly
/// available through the bridge:
///
/// ```compile_fail
/// use ommx_pyo3_bridge::{ProtobufV1, PySolution, Target};
/// use pyo3::prelude::*;
/// fn lossy(py: Python<'_>, target: Target<ProtobufV1>, value: ommx::Solution) -> PyResult<PySolution> {
///     target.transfer(py, value)
/// }
/// ```
pub trait Export<P: TransferProtocol, T: TransferVia<P>> {
    /// Export the value, reporting any representation limitations.
    fn export(self) -> ommx::Result<T::Payload>;
}

/// A loaded Python SDK that explicitly supports protocol `P`.
///
/// Only [`resolve_target`] constructs a target, after checking the SDK's
/// declaration. The protocol and SDK module stay fixed. Each transfer resolves
/// its type-specific receiver on that module without probing protocols again.
pub struct Target<P: TransferProtocol> {
    module: Py<PyModule>,
    marker: PhantomData<P>,
}

impl<P: TransferProtocol> Target<P> {
    /// Export a Rust value and reconstruct its Python object with protocol `P`.
    ///
    /// The Python return type `T` is inferred from the caller's return type or
    /// can be specified with `transfer::<PyInstance>`. Unsupported type/protocol
    /// pairs and source types fail to compile. The target can transfer multiple
    /// types supported by the same protocol.
    ///
    /// A missing or invalid receiver raises `ImportError` before export. Export
    /// and import failures preserve the original cause and identify the type,
    /// protocol, and stage. No failure triggers another protocol probe.
    ///
    /// ```compile_fail
    /// use ommx_pyo3_bridge::{ProtobufV1, Target};
    /// use pyo3::prelude::*;
    /// fn unsupported(py: Python<'_>, target: Target<ProtobufV1>) {
    ///     target.transfer::<String>(py, ommx::Function::default());
    /// }
    /// ```
    pub fn transfer<T: TransferVia<P>>(
        &self,
        py: Python<'_>,
        value: impl Export<P, T>,
    ) -> PyResult<T> {
        let module = self.module.bind(py);
        let receiver = T::receiver(module).map_err(|source| {
            let error = PyImportError::new_err(format!(
                "Python OMMX advertises {}, but its receiver for {} is unavailable",
                P::NAME,
                T::PYTHON_NAME,
            ));
            error.set_cause(module.py(), Some(source));
            error
        })?;
        if !receiver.bind(module.py()).is_callable() {
            return Err(PyImportError::new_err(format!(
                "Python OMMX receiver for {} over {} is not callable",
                T::PYTHON_NAME,
                P::NAME,
            )));
        }
        let payload = value.export().map_err(|source| {
            transfer_error::<P, T>(py, "export", PyRuntimeError::new_err(format!("{source:#}")))
        })?;
        T::import(payload, receiver.bind(py))
            .map_err(|source| transfer_error::<P, T>(py, "import", source))
    }
}

fn transfer_error<P: TransferProtocol, T: TransferVia<P>>(
    py: Python<'_>,
    stage: &str,
    source: PyErr,
) -> PyErr {
    let error = PyRuntimeError::new_err(format!(
        "OMMX bridge failed to transfer {} using {} during {stage}: {source}",
        T::PYTHON_NAME,
        P::NAME,
    ));
    error.set_cause(py, Some(source));
    error
}

/// Load Python OMMX and probe its explicit support for one protocol.
///
/// Returns `None` only when the SDK does not advertise `P`; callers can then try
/// another protocol. Import failures or a missing/malformed declaration return
/// an error. Unknown IDs are ignored. Each probe reads the declaration anew;
/// a successful target retains that SDK module and never renegotiates.
pub fn resolve_target<P: TransferProtocol>(py: Python<'_>) -> PyResult<Option<Target<P>>> {
    let module = PyModule::import(py, "ommx")?;
    let declaration = module
        .getattr("_ommx_rust")
        .and_then(|module| module.getattr(crate::protocol::SUPPORTED_PROTOCOLS))
        .and_then(|declare| declare.call0())
        .and_then(|ids| ids.extract::<Vec<u32>>());
    let ids = declaration.map_err(|source| {
        let error = PyImportError::new_err(format!(
            "Python OMMX must declare supported transfer protocols through ommx._ommx_rust.{}()",
            crate::protocol::SUPPORTED_PROTOCOLS,
        ));
        error.set_cause(py, Some(source));
        error
    })?;
    if !ids.contains(&(P::ID as u32)) {
        return Ok(None);
    }
    Ok(Some(Target {
        module: module.unbind(),
        marker: PhantomData,
    }))
}
