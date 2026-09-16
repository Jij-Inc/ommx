# `ommx-pyo3-bridge`

`ommx-pyo3-bridge` converts values from the Rust `ommx` crate into the
canonical classes provided by the installed Python `ommx` package. It is for
independently built PyO3 extension modules that cannot safely pass Rust values
or private PyO3 wrapper types across a shared-library boundary.

The default `sender` feature provides Rust SDK v3 conversions and
`pyo3-stub-gen` return types. A receiving Python SDK disables it:

```toml
ommx-pyo3-bridge = { version = "=3.0.0-beta.6", default-features = false }
```

Without `sender`, the crate depends only on PyO3: protocol IDs, receiver
configurations, registration, and SDK exception lookup remain available.
It does not depend on `ommx`, protobuf codecs, or `pyo3-stub-gen`. The receiving
SDK supplies its own parsers and constructors, so a Rust SDK v2 receiver does
not need to compile or pass through Rust SDK v3. Producers use the default
features for `resolve_target`, `Target::transfer`, and the `Py*` return types.

Select a supported protocol and transfer the value before returning it:

```rust,no_run
# #[cfg(feature = "sender")]
# mod example {
# use super::*;
use ommx_pyo3_bridge::{
    resolve_target, BridgeError, ProtobufV2, PyFunction, TransferProtocolId,
};
use pyo3::prelude::*;

#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
fn objective(py: Python<'_>) -> PyResult<PyFunction> {
    let target = resolve_target::<ProtobufV2>(py)?.ok_or_else(|| {
        BridgeError::no_supported_protocol(py, &[TransferProtocolId::ProtobufV2])
    })?;
    target.transfer(py, ommx::Function::default())
}
# }
```

At runtime this return value is an `ommx.Function`. `pyo3-stub-gen` also emits
`ommx.Function` in the generated stub. The crate provides the same pattern for
`PyConstraint`, `PyDecisionVariable`, `PyInstance`, `PyParametricInstance`,
`PySolution`, and `PySampleSet`. Each wrapper owns a completed Python object
and is constructed through `Target::transfer`.

## Sender and receiver contract

This crate serves two independently built extensions: a producer such as
JijModeling, and the installed OMMX Python SDK. They communicate through a
Python callable contract.

| Side | Responsibility |
| --- | --- |
| Sender / transfer API | Read `ommx._ommx_rust._bridge_supported_protocols()`, select a protocol, and call its named receiver functions under `ommx._ommx_rust` with the specified arguments. |
| Python SDK / receiver API | Supply factories that parse protocol payloads with the receiving SDK and construct its canonical classes. `register_receivers` implements the required functions and derives the supported-protocol list from those configurations. |

The sender relies on those functions and their behavior, not on how the SDK
implements them. A receiver can implement the same contract without using this
crate. The receiver API is an implementation aid: keeping both sides' endpoint
names, call signatures, and registration in the bridge crate makes their
definitions easier to maintain together. Payload parsing and feature validation
belong to the receiving SDK; Rust SDK serialization belongs to the optional
sender implementation.

The contract specifies payload representations, ownership, and canonical
Python return types such as `ommx.Instance`. It does not require public Python
constructors or `from_v1_bytes` / `from_v2_bytes` methods. The SDK's factories
must construct the promised Python classes and preserve the supplied domain
data; the sender does not inspect their private Rust implementation or validate
the returned Python class at runtime. Rust values and factory pointers stay
inside their respective extensions.

Compatibility is determined by the selected protocol, not by matching SDK or
bridge crate version numbers:

- Once a protocol is published, its ID and callable contract must retain their
  meaning. The underscore-prefixed receiver functions are internal to the
  bridge, but independently distributed senders depend on them. Incompatible
  contract changes, including adding required receiver functions, need a new
  protocol ID.
- Protocol support covers the transfer representation, not every payload
  feature. Serializers and parsers decide whether a particular value is
  supported. A failed transfer never automatically retries another protocol.
- A receiver can register old and new protocol configurations together during
  migration. Removing support breaks senders that rely on that protocol and
  must be treated as an explicit compatibility change.

The two extensions may use different bridge crate versions while honoring the
same runtime contract. Within each extension, the SDK, bridge, and PyO3 Rust
dependencies must resolve compatible types.
This release accepts PyO3 0.27.2 through the 0.29 release line. The `sender`
feature uses `pyo3-stub-gen` 0.23; a receiving SDK can use its own stub generator
version without taking a dependency on the bridge's generator.

## Select a transfer target before compilation

Probe one protocol at a time with `resolve_target::<P>`. The order of the
caller's `if` statements expresses its preference:

```rust,no_run
# #[cfg(feature = "sender")]
# mod example {
# use super::*;
use ommx::{Constraint, ConstraintID, DecisionVariable, Instance, Sense};
use ommx_pyo3_bridge::{
    resolve_target, BridgeError, ProtobufV1, ProtobufV2, PyInstance, TransferProtocolId,
};
use pyo3::prelude::*;
use std::collections::BTreeMap;

#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
fn compile(py: Python<'_>) -> PyResult<PyInstance> {
    if let Some(target) = resolve_target::<ProtobufV2>(py)? {
        let mut instance: Instance = compile_regular();
        // Promote x + y = 1 to a first-class one-hot constraint, requiring v2.
        let source = ConstraintID::from(23);
        let promoted = instance.promote_one_hot(&[source].into_iter().collect());
        promoted[&source]
            .as_ref()
            .expect("the example is exactly one-hot");
        return target.transfer(py, instance);
    }
    if let Some(target) = resolve_target::<ProtobufV1>(py)? {
        // Keep x + y = 1 as a regular constraint in the same Rust domain model.
        let instance: Instance = compile_regular();
        return target.transfer(py, instance);
    }
    Err(BridgeError::no_supported_protocol(
        py,
        &[TransferProtocolId::ProtobufV2, TransferProtocolId::ProtobufV1],
    ))
}

fn compile_regular() -> Instance {
    // Minimize x subject to x + y = 1, with binary x and y.
    let equality =
        ((ommx::linear!(7) + ommx::linear!(8)).unwrap() + ommx::coeff!(-1.0)).unwrap();
    Instance::builder()
        .sense(Sense::Minimize)
        .objective(ommx::Function::from(ommx::linear!(7)))
        .decision_variables(BTreeMap::from([
            (7.into(), DecisionVariable::binary()),
            (8.into(), DecisionVariable::binary()),
        ]))
        .constraints(BTreeMap::from([(
            23.into(),
            Constraint::equal_to_zero(equality.into()),
        )]))
        .build()
        .unwrap()
}
# }
```

Both paths construct `ommx::Instance`. `Target<ProtobufV1>::transfer` calls
its checked `to_v1_bytes()` serializer; `Target<ProtobufV2>::transfer` calls
`to_v2_bytes()`. The promoted instance contains a first-class one-hot constraint
that v1 cannot represent: sending it over ProtobufV1 would fail during export.
The V1 path keeps the regular equality so it can be serialized without losing
the model's meaning. The compiler chooses this representation before transfer;
the bridge does not lower special constraints or retry after an export error.

The stub is `def compile() -> ommx.Instance`. Protocols belong to target
selection and transfer; the return type is simply `PyInstance` on both paths.
`Target<P>::transfer` infers the return type `T`, or accepts it explicitly as
`transfer::<PyInstance>`. It requires `T: TransferVia<P>` and an input implementing
`Export<P, T>`, so unsupported type/protocol pairs and Rust inputs fail to
compile. `Export` is open to downstream source types; protocols and Python
type/protocol pairs are bridge-owned. A target can transfer multiple supported
types through the same protocol without probing the SDK again.

`resolve_target` imports `ommx` and calls the binding-private declaration
`ommx._ommx_rust._bridge_supported_protocols() -> list[int]`. It also loads the
SDK's `ommx._ommx_rust.BridgeError` class. It returns `Some`
with a `Target<P>` if `P` is advertised, or `None` otherwise. Each probe reads
the declaration anew; callers decide which protocol to try next and what to do
if none are supported. Errors propagate with `?` instead of being interpreted
as lack of support.

The examples raise `BridgeError` when the loaded SDK supports none of the
caller's requested protocols. This is a caller-owned compatibility decision
after a successful import and protocol probe.

A successful target retains the selected protocol, loaded SDK module, and its
exception class.
`Target::transfer` resolves the type-specific reconstruction callable on that
module, exports the payload, and imports it. Receiver lookup errors therefore
occur at transfer, when the return type is known. Replacing `sys.modules["ommx"]`
does not redirect an existing target, but modifying receivers on the retained
module affects subsequent transfers. The target never rechecks protocol support.

The returned `PyInstance` internally owns the completed Python object. Returning
it through PyO3 does not repeat the transfer. The wrapper holds a single owned
Python reference without an intermediate Rust value.

Pass Rust SDK values or complete protobuf messages to `transfer`; `Py*`
wrappers are output values. Components with owner-side data use tuples:
`(Constraint, ConstraintContext)` and `(VariableID, DecisionVariable, ModelingLabel)`.

There is no SDK version comparison, attribute-based capability inference, or
global negotiation cache. Unknown IDs are ignored. A missing SDK, exception
class, declaration API, or receiver raises `ImportError`. Non-callable bridge
APIs also raise `ImportError`. Invalid declarations and export/import failures
raise the SDK's `BridgeError`, with transfer failures identifying
the Python type, protocol, and stage, with the underlying exception in
`__cause__`. Failure after selection never retries another protocol. Payload
features are validated by the serializers and parsers, not by negotiation.

## Bridge errors

The Python SDK defines `ommx.BridgeError`, a `RuntimeError` subclass used by
senders and receivers. Invalid protocol declarations, receiver registration
errors, and payload/transfer failures use this class. After all acceptable
protocol probes return `None`, callers use
`BridgeError::no_supported_protocol(py, &requested)` to report the mismatch.
It includes the requested protocol names in caller order without probing the
SDK again. `BridgeError::new_err(py, message)` is available for other
caller-defined bridge failures.
Transfer wrappers retain the original Python exception in `__cause__`.
Python argument-binding errors and exceptions raised directly by SDK factories
keep their original types; a sender wraps factory failures during transfer.

The SDK supplies its exception class to `register_receivers`, which publishes it
as `ommx._ommx_rust.BridgeError`. The SDK re-exports that class as
`ommx.BridgeError`; the sender retrieves the actual Python class from the SDK.
Independently built senders do not define their own exception classes, so
`except ommx.BridgeError` catches their transfer failures too. SDK imports and
missing bridge APIs raise `ImportError` because the expected SDK interface is
unavailable. No Rust exception object or layout is shared across extensions.

Python v3 advertises both protocols. A future Python v2 compatibility release
must explicitly advertise the contracts it implements and provide the same
top-level class aliases and receivers. This change does not by itself make
existing Python v2 releases compatible with negotiated transfers.

## Transfer contracts

`TransferProtocolId` is the bridge's registry. IDs are stable and never reused;
their numeric order does not express preference. A protocol defines payloads,
receiver signatures, and ownership, independently of SDK release numbers.

| ID | Rust marker | Root representation |
| --- | --- | --- |
| 1 | `ProtobufV1` | `ommx.v1` protobuf messages |
| 2 | `ProtobufV2` | `ommx.v2` protobuf messages |

The root types are `Instance`, `ParametricInstance`, `Solution`, and `SampleSet`.
They use private receivers on `ommx._ommx_rust`, named
`_bridge_protobuf_v1_<kind>_from_bytes(bytes)` and
`_bridge_protobuf_v2_<kind>_from_bytes(bytes)`, where `<kind>` is `instance`,
`parametric_instance`, `solution`, or `sample_set`. Public Python class
constructors and decoding methods are not part of the transfer contract.

`ommx::Instance` and `ommx::ParametricInstance` export over ProtobufV1
through their checked v1 serializers. Special constraints that cannot be
represented in v1 cause an export error. Domain `Solution` and `SampleSet` only
export
over ProtobufV2: their existing v1 conversions discard special-constraint
results, so callers choosing ProtobufV1 must explicitly construct v1 messages.
All four domain roots export over ProtobufV2.

For callers that already hold complete raw `ommx::v1` roots, ProtobufV1 also
accepts those messages without parsing them into the Rust v3 domain model.
This preserves advisory `ConstraintHints` until they reach the receiver. A v3
receiver may ignore those hints, as allowed by the schema.

Components use binding-private receivers on `ommx._ommx_rust`:

| Type | ProtobufV1 receiver and arguments | ProtobufV2 receiver and arguments |
| --- | --- | --- |
| `Function` | `_bridge_protobuf_v1_function_from_bytes(v1_function)` | `_bridge_protobuf_v2_function_from_bytes(v1_function)` |
| `Constraint` | `_bridge_protobuf_v1_constraint_from_bytes(v1_constraint)` | `_bridge_protobuf_v2_constraint_from_bytes(v2_regular_constraint, v2_context)` |
| `DecisionVariable` | `_bridge_protobuf_v1_decision_variable_from_bytes(v1_variable)` | `_bridge_protobuf_v2_decision_variable_from_bytes(id, v2_variable, v2_label)` |

All arguments other than `id` are protobuf bytes. `Function`'s schema remains
in `ommx.v1` even in normalized roots. Component payloads do not create new
persistence roots.

ProtobufV1 accepts raw v1 messages for all three components, plus Rust
`Function` and `(VariableID, DecisionVariable, ModelingLabel)`. The v1
`Constraint` source
must carry its original ID and label; the bridge does not invent an ID for a
detached `PyConstraint`. Python v3 reconstructs a detached constraint, whose
identity will be assigned by its eventual owning Instance. A v1 variable with
a fixed value is rejected by the detached receiver because fixed values belong
to an Instance. ProtobufV2 accepts Rust `Function`, `(Constraint, ConstraintContext)`,
and `(VariableID, DecisionVariable, ModelingLabel)`.

These contracts cover representation and ownership, not feature sets. An older
parser can reject a protobuf variant it does not understand even when its
protocol is selected. Existing IDs and endpoint meanings must not be changed
to reinterpret such payloads. A future shared-memory or Arrow transfer would
get a new marker, ID, and `TransferVia::Payload` type; payloads are not required
to be byte buffers and own their resources until import or drop.

## Configure Python SDK receivers

The Python SDK supplies a [`ProtobufV1ReceiverConfig`] and/or
[`ProtobufV2ReceiverConfig`]. These are distinct concrete types, each defining
the required factory signatures for its protocol. Both currently require
factories for all seven canonical classes: `Function`, `Constraint`,
`DecisionVariable`, `Instance`, `ParametricInstance`, `Solution`, and
`SampleSet`.

Factories receive the protocol's wire arguments unchanged. ProtobufV1 factories
accept `(Python, &[u8])` containing complete v1 messages. ProtobufV2 uses the
same shape for roots and Function, `(Python, &[u8], &[u8])` for Constraint,
and `(Python, u64, &[u8], &[u8])` for DecisionVariable, matching the argument
table above. Borrowed bytes remain valid for the call; factories return owned
Python objects. Future protocols may use representations other than bytes.

Each SDK factory parses and validates with its own SDK, then constructs the
promised Python class. The bridge does not decode into an intermediate Rust
SDK domain model. In particular, a v2 factory can preserve complete v1
`ConstraintHints` and component IDs without going through a v3 parser.
A v3 factory uses the v3 parsers, which may ignore advisory v1 hints.
Factories must preserve domain data required by the contract, reject unsupported
features through their parsers, and classify malformed or unsupported payloads
using the SDK's `BridgeError`. Python-owned constructor exceptions propagate
unchanged; senders wrap failures with transfer context and `__cause__`.

Collect the concrete configurations as [`ReceiverConfig`] enum values and pass
the complete list to [`register_receivers`] once during SDK initialization:

```rust,no_run
use ommx_pyo3_bridge::{
    register_receivers, ProtobufV1ReceiverConfig, ProtobufV2ReceiverConfig, ReceiverConfig,
};
use pyo3::{prelude::*, types::PyType};

fn install_receivers(
    module: &Bound<'_, PyModule>,
    bridge_error: &Bound<'_, PyType>,
    v1: ProtobufV1ReceiverConfig,
    v2: ProtobufV2ReceiverConfig,
) -> PyResult<()> {
    let configs: Vec<ReceiverConfig> = vec![v1.into(), v2.into()];
    register_receivers(module, bridge_error, configs)
}
```

The list determines the supported protocols and their declaration order;
there is no separate list of protocol IDs to keep in sync. Registration
receives the SDK-defined `BridgeError` class alongside that list and uses it
for registration failures. Sender-side errors use the same class via Python lookup.
Registration prepares all private receivers and publishes the support
declaration last.
Duplicate protocols, repeated registration, and occupied endpoint names
raise `BridgeError` before any module attributes are changed. An empty list
advertises no supported protocols. Registration also exposes the shared
`BridgeError` class. No public Python domain classes or decoding
methods need to be present for registration.

Sender lookups and receiver registration use the same bridge-owned endpoint
definitions. Receiver factories are retained locally by private bound Python
methods, with no global configuration and no Rust object sharing between
independent extension modules. Call signatures, registration, and sender error
conversion belong to the bridge crate. Each Python SDK owns payload decoding,
feature validation, its canonical classes, and public decoding APIs. The bridge
does not look up or call public domain constructors or decoding methods.
