# OMMX Python SDK 2.9.x

## Bug Fixes

### Preserve annotations in protobuf payloads ([#1262](https://github.com/Jij-Inc/ommx/pull/1262))

`Instance`, `ParametricInstance`, `Solution`, and `SampleSet` now retain user
annotations and typed metadata through `to_bytes()` / `from_bytes()`. Instance
titles, licenses, datasets, authors, and creation times are stored in protobuf
Description fields; solution and sampling provenance is stored in ProcessMetadata.
The existing mutable `annotations` dictionaries and annotation helpers remain available.

```python
from ommx import Instance

instance = Instance.empty()
instance.title = "My model"
instance.add_user_annotation("source", "python")
restored = Instance.from_bytes(instance.to_bytes())
assert restored.title == "My model"
assert restored.get_user_annotation("source") == "python"
```

Model conversions, partial evaluation, parameter materialization, and sample
projection also retain annotations. Artifact readers merge legacy descriptor-only
annotations, with protobuf metadata taking precedence when both sources contain
the same key. Variable and constraint counts remain Artifact descriptor annotations.

Custom annotations must use a user or third-party namespace; the
`org.ommx.v1.*` namespace is reserved for OMMX metadata. Serialization rejects
unknown keys in that namespace. The added protobuf fields use the same field
numbers as SDK v3 and keep `format_version = 0`. Older SDKs can still read the
mathematical model, but may discard the added metadata when decoding and re-encoding it.

The `ommx.v1` protobuf definitions are shared with SDK v3, including the
`Function.Expression` schema. SDK 2.x continues to reject this unsupported
function representation when loading a model.

### Batch integer encoding before penalty construction (2.9.2, [#1254](https://github.com/Jij-Inc/ommx/pull/1254))

`Instance.log_encode()` processes all selected integers in one batch, validating
every bound before changing the instance. Acyclic substitution resolves assignment
dependencies and traverses each polynomial's terms once.

`to_qubo()` and `to_hubo()` now encode integers after slack conversion and before
constructing squared penalties. Constraints removed by the penalty phase store
encoded expressions; their values and feasibility are still evaluated using the
reconstructed decision variables. To select a different order, call the
individual conversion methods explicitly.

### Faster HiGHS model construction (2.9.0, [#1239](https://github.com/Jij-Inc/ommx/pull/1239))

The HiGHS adapter now assembles long linear objectives and constraints without
repeatedly copying the growing expression. Constructing
`OMMXHighsAdapter(instance)` only builds the model; it no longer runs the solver
before adding constraints. Use `OMMXHighsAdapter.solve(instance)` for the complete
workflow, or call `adapter.solver_input.run()` to solve a constructed model.
Constant objectives also retain their value and minimization/maximization sense
in the HiGHS model.

## New Features

### Receive Rust SDK v3 models over the V1 bridge (2.9.0, [#1226](https://github.com/Jij-Inc/ommx/pull/1226))

Rust producers can negotiate ProtobufV1 and transfer models into the existing
Python SDK 2.x classes, including regular constraint formulations and
`ConstraintHints` for legacy solver adapters. The receiving SDK validates the
payload and rejects unsupported function representations.

The seven bridge types are now also available at the top level:
`ommx.Function`, `Constraint`, `DecisionVariable`, `Instance`,
`ParametricInstance`, `Solution`, and `SampleSet`. Each is the same class as its
`ommx.v1` counterpart; existing imports and solver APIs continue to work.
`ommx.BridgeError` identifies bridge protocol and payload failures.

This SDK advertises ProtobufV1 only. A producer chooses its mathematical
representation before transfer; the bridge does not lower or promote constraints.

## Improvements

### Remove Python upper bounds from solver adapters (2.9.1, [#1246](https://github.com/Jij-Inc/ommx/pull/1246))

The HiGHS, PySCIPOpt, and OpenJij adapters now declare `requires-python = ">=3.10"`
without the previous `<3.14` upper bound. Their package metadata no longer blocks
installation on Python 3.14. Installation still requires compatible solver and
dependency packages for the selected Python version and platform.
