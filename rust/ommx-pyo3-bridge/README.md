# `ommx-pyo3-bridge`

`ommx-pyo3-bridge` converts values from the Rust `ommx` crate into the
canonical classes provided by the installed Python `ommx` package. It is for
independently built PyO3 extension modules that cannot safely pass Rust values
or private PyO3 wrapper types across a shared-library boundary.

Select a supported protocol and transfer the value before returning it:

```rust,no_run
use ommx_pyo3_bridge::{resolve_target, ProtobufV2, PyFunction};
use pyo3::{exceptions::PyImportError, prelude::*};

#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
fn objective(py: Python<'_>) -> PyResult<PyFunction> {
    let target = resolve_target::<ProtobufV2>(py)?
        .ok_or_else(|| PyImportError::new_err("Python OMMX does not support ProtobufV2"))?;
    target.transfer(py, ommx::Function::default())
}
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
| Python SDK / receiver API | Supply factories through protocol-specific configurations. `register_receivers` implements the required functions and derives the supported-protocol list from those configurations. |

The sender relies on those functions and their behavior, not on how the SDK
implements them. A receiver can implement the same contract without using this
crate. The receiver API is an implementation aid: keeping both sides' endpoint
names, call signatures, and codecs in the bridge crate makes their definitions
easier to maintain together.

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
This release accepts PyO3 0.27.2 through the 0.29 release line and uses
`pyo3-stub-gen` 0.23.

## Select a transfer target before compilation

Probe one protocol at a time with `resolve_target::<P>`. The order of the
caller's `if` statements expresses its preference:

```rust,no_run
use ommx_pyo3_bridge::{resolve_target, ProtobufV1, ProtobufV2, PyInstance};
use pyo3::{exceptions::PyImportError, prelude::*};

#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
fn compile(py: Python<'_>) -> PyResult<PyInstance> {
    if let Some(target) = resolve_target::<ProtobufV2>(py)? {
        // Compile the v3 model, including special-constraint promotions.
        let instance: ommx::Instance = compile_normalized();
        return target.transfer(py, instance);
    }
    if let Some(target) = resolve_target::<ProtobufV1>(py)? {
        // Compile ordinary constraints and advisory ConstraintHints.
        let instance: ommx::v1::Instance = compile_legacy();
        return target.transfer(py, instance);
    }
    Err(PyImportError::new_err("No supported OMMX transfer protocol"))
}
# fn compile_normalized() -> ommx::Instance { unimplemented!() }
# fn compile_legacy() -> ommx::v1::Instance { unimplemented!() }
```

The stub is `def compile() -> ommx.Instance`. Protocols belong to target
selection and transfer; the return type is simply `PyInstance` on both paths.
`Target<P>::transfer` infers the return type `T`, or accepts it explicitly as
`transfer::<PyInstance>`. It requires `T: TransferVia<P>` and an input implementing
`Export<P, T>`, so unsupported type/protocol pairs and Rust inputs fail to
compile. `Export` is open to downstream source types; protocols and Python
type/protocol pairs are bridge-owned. A target can transfer multiple supported
types through the same protocol without probing the SDK again.

`resolve_target` imports `ommx` and calls the binding-private declaration
`ommx._ommx_rust._bridge_supported_protocols() -> list[int]`. It returns `Some`
with a `Target<P>` if `P` is advertised, or `None` otherwise. Each probe reads
the declaration anew; callers decide which protocol to try next and what to do
if none are supported. Errors propagate with `?` instead of being interpreted
as lack of support.

A successful target retains the selected protocol and loaded SDK module.
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
global negotiation cache. Unknown IDs are ignored. A missing or malformed
declaration, or an advertised but unavailable receiver
raises `ImportError`. Export/import failures raise `RuntimeError` identifying
the Python type, protocol, and stage, with the underlying exception in
`__cause__`. Failure after selection never retries another protocol. Payload
features are validated by the serializers and parsers, not by negotiation.

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
ProtobufV1 accepts complete raw `ommx::v1`
roots without parsing them into the Rust v3 domain model. This preserves
advisory `ConstraintHints` until they reach the receiver. A v3 receiver may
ignore those hints, as allowed by the schema.

`ommx::Instance` and `ommx::ParametricInstance` also export over ProtobufV1
through their checked v1 serializers. Special constraints that cannot be
represented in v1 cause an export error. Domain `Solution` and `SampleSet` only
export
over ProtobufV2: their existing v1 conversions discard special-constraint
results, so callers choosing ProtobufV1 must explicitly construct v1 messages.
All four domain roots export over ProtobufV2.

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

Factories run after the protocol's Rust SDK parser and construct the Python
objects from the parsed values. Complete constraint context, variable IDs and
labels, and root-owned data are preserved. The same factory implementation can
be supplied to both protobuf configurations. Future protocol configurations
can require different input types or ownership rules.

Collect the concrete configurations as [`ReceiverConfig`] enum values and pass
the complete list to [`register_receivers`] once during SDK initialization:

```rust,no_run
use ommx_pyo3_bridge::{
    register_receivers, ProtobufV1ReceiverConfig, ProtobufV2ReceiverConfig, ReceiverConfig,
};
use pyo3::prelude::*;

fn install_receivers(
    module: &Bound<'_, PyModule>,
    v1: ProtobufV1ReceiverConfig,
    v2: ProtobufV2ReceiverConfig,
) -> PyResult<()> {
    let configs: Vec<ReceiverConfig> = vec![v1.into(), v2.into()];
    register_receivers(module, configs)
}
```

The list determines the supported protocols and their declaration order;
there is no separate list of protocol IDs to keep in sync. Registration
prepares all private receivers and publishes the support declaration last.
Duplicate protocols, repeated registration, and occupied endpoint names
raise `ImportError` before any module attributes are changed. An empty list
advertises no supported protocols. No public Python classes or decoding
methods need to be present for registration.

Sender lookups and receiver registration use the same bridge-owned endpoint
definitions. Receiver factories are retained locally by private bound Python
methods, with no global configuration and no Rust object sharing between
independent extension modules. Call signatures, decoding through the core
parsers, and bridge error conversion belong to the bridge crate. The Python
SDK retains ownership of its canonical classes and public decoding APIs;
bridge transfers do not look up or call those public methods.
