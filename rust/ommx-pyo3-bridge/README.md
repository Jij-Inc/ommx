# `ommx-pyo3-bridge`

`ommx-pyo3-bridge` converts values from the Rust `ommx` crate into the
canonical classes provided by the installed Python `ommx` package. It is for
independently built PyO3 extension modules that cannot safely pass Rust values
or private PyO3 wrapper types across a shared-library boundary.

Use the bridge wrapper as the direct return type of a PyO3 function:

```rust,no_run
use ommx_pyo3_bridge::PyFunction;
use pyo3::prelude::*;

#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
fn objective() -> PyFunction {
    ommx::Function::default().into()
}
```

At runtime this return value is an `ommx.Function`. `pyo3-stub-gen` also emits
`ommx.Function` in the generated stub. The crate provides the same pattern for
detached constraints, detached decision variables, and instances through
`PyConstraint`, `PyDecisionVariable`, `PyInstance`, `PyParametricInstance`,
`PySolution`, and `PySampleSet`.

The consuming extension and the bridge must resolve a compatible PyO3 version.
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
it through PyO3 does not repeat the transfer. Cloning a completed wrapper shares
that Python object and does not require the GIL.

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
Their receivers are `ommx.<Type>.from_v1_bytes(bytes)` and
`ommx.<Type>.from_v2_bytes(bytes)`. ProtobufV1 accepts complete raw `ommx::v1`
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
| `Function` | `_bridge_protobuf_v1_function_from_bytes(v1_function)` | `_pyo3_bridge_v0_function_from_bytes(v1_function)` |
| `Constraint` | `_bridge_protobuf_v1_constraint_from_bytes(v1_constraint)` | `_pyo3_bridge_v0_constraint_from_bytes(v2_regular_constraint, v2_context)` |
| `DecisionVariable` | `_bridge_protobuf_v1_decision_variable_from_bytes(v1_variable)` | `_pyo3_bridge_v0_decision_variable_from_bytes(id, v2_variable, v2_label)` |

All arguments other than `id` are protobuf bytes. ProtobufV2 reuses the fixed
component contracts already provided by bridge v0; `Function`'s schema remains
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

The Python SDK configures [`ReceiverConfig`] with its supported protocols, a
`legacy_v0` flag, and three factories for its canonical `Function`, `Constraint`,
and `DecisionVariable` classes. The factories receive parsed Rust values,
including the complete constraint context or variable ID and label. They only
construct the SDK's Python objects; they do not define endpoint names, decode
payloads, or implement the support declaration.

Call `ReceiverConfig::register` after registering the SDK's `Instance`,
`ParametricInstance`, `Solution`, and `SampleSet` classes on the extension
module. Registration checks their required public `from_v1_bytes` and
`from_v2_bytes` decoders before adding the configured component endpoints and
support declaration. ProtobufV2 reuses the fixed v0 component endpoints even
when `legacy_v0` is false; the flag also enables them for SDKs that do not
advertise ProtobufV2.

Sender lookups and receiver registration use the same bridge-owned endpoint
definitions. Receiver factories are retained locally by private bound Python
methods, with no global configuration and no Rust object sharing between
independent extension modules. Call signatures, decoding through the core
parsers, and bridge error conversion belong to the bridge crate. The Python
SDK retains ownership of its canonical classes and public root decoding APIs.

## Existing bridge v0 returns

The bridge serializes each value to protobuf bytes, passes those bytes through
the Python/PyO3 ABI, and asks the installed Python OMMX package to reconstruct
its canonical class. These payloads are an ephemeral implementation detail of
the bridge, not new persistence roots for component values.

The installed Python SDK owns the binding-private reconstruction endpoints
because it owns the canonical Python classes. These endpoints are production
bridge capabilities, not user-facing component serialization APIs.

Direct `PyFunction`, `PyInstance`, and other `Py*` returns retain internal
bridge protocol v0 to identify the required endpoint
signatures and payload interpretations. Protocol v0 is not a new persistence
format, and the bridge performs no protocol negotiation. Rust and Python OMMX
releases remain independently versioned, with no guarantee that arbitrary
release pairs are compatible. Callers must install a Python OMMX release that
provides the exact v0 capabilities expected by this crate. A missing endpoint
produces an `ImportError` with the required capability name.

The endpoint may exist while the sender uses a protobuf variant introduced by
a newer OMMX release. In that case an older Python receiver rejects the payload
as incompatible; callers must upgrade the Python OMMX package to a release that
understands the sender's Function variants.

When a Python SDK exposes a v0 capability, its endpoint signature and payload
interpretation have the fixed meaning recorded in
`tests/data/protocol_v0.json`. An incompatible change must add a new protocol
version instead of reinterpreting v0. This does not require every future Python
SDK release to retain v0. Wrappers constructed with `new` or `From` use v0;
use `resolve_target` and `Target::transfer` for explicit target selection.
