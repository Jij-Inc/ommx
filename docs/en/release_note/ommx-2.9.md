# OMMX Python SDK 2.9.x

## Unreleased

### Instance snapshot read APIs ([#1244](https://github.com/Jij-Inc/ommx/pull/1244))

The next 2.x release adds two methods also available in the v3 development line:

- `Instance.fixed_decision_variables() -> dict[int, float]` returns explicitly
  stored substituted values by variable ID. Coincident bounds alone do not add
  a variable to the result.
- `Instance.regular_constraint_ids() -> set[int]` returns active regular
  constraint IDs, excluding removed constraints. OneHot/SOS1 hints leave their
  source constraints active, so those IDs remain included.

```python
fixed = instance.fixed_decision_variables()
constraints = {
    cid: instance.get_constraint_by_id(cid)
    for cid in instance.regular_constraint_ids()
}
```

Both methods return independent snapshots, including empty containers when
there are no matching entries. Reading or modifying the results does not
change the instance. These additions are not part of the released SDK 2.9.0.

## Bug Fixes

### Faster HiGHS model construction ([#1239](https://github.com/Jij-Inc/ommx/pull/1239))

The HiGHS adapter now assembles long linear objectives and constraints without
repeatedly copying the growing expression. Constructing
`OMMXHighsAdapter(instance)` only builds the model; it no longer runs the solver
before adding constraints. Use `OMMXHighsAdapter.solve(instance)` for the complete
workflow, or call `adapter.solver_input.run()` to solve a constructed model.
Constant objectives also retain their value and minimization/maximization sense
in the HiGHS model.

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
