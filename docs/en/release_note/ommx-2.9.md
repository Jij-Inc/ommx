# OMMX Python SDK 2.9.x

## Bug Fixes

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
