# OMMX Python SDK 2.9.x

## New Features

### Receive Rust SDK v3 models over the V1 bridge ([#1226](https://github.com/Jij-Inc/ommx/pull/1226))

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
