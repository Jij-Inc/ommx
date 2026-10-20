# OMMX adapter for SCIP

This package provides an adapter for the [SCIP](https://www.scipopt.org/) from [OMMX](https://github.com/Jij-Inc/ommx)

## Usage

`ommx-pyscipopt-adapter` can be installed from PyPI as follows:

```bash
pip install ommx-pyscipopt-adapter
```

SCIP can be used through `ommx-pyscipopt-adapter` by using the following:

```python markdown-code-runner
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter
from ommx.v1 import Instance, DecisionVariable

x1 = DecisionVariable.integer(1, lower=0, upper=5)
ommx_instance = Instance.from_components(
    decision_variables=[x1],
    objective=x1,
    constraints=[],
    sense=Instance.MINIMIZE,
)

# Create `ommx.v1.Solution` from the `pyscipot.Model`
ommx_solution = OMMXPySCIPOptAdapter.solve(ommx_instance)

print(ommx_solution)
```

## Initial solution by variable name

Pass values grouped by variable name and tuple subscripts to `initial_state`.
The adapter resolves them against the instance being solved, so the same
values can be reused when variable IDs change.

```python markdown-code-runner
from ommx import DecisionVariable, Instance, NamedState
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

x = DecisionVariable.integer(31, lower=0, upper=5, name="x", subscripts=[0])
y = DecisionVariable.integer(107, lower=0, upper=5, name="x", subscripts=[1])
instance = Instance.from_components(
    decision_variables=[x, y], objective=2 * x + y,
    constraints=[x + y <= 5], sense=Instance.MAXIMIZE,
)
initial_values: NamedState = {"x": {(0,): 1.0, (1,): 4.0}}
solution = OMMXPySCIPOptAdapter.solve(instance, initial_state=initial_values)
assert solution.state.entries == {x.id: 5.0, y.id: 0.0}

# Explicit conversion is also available when you need a State.
state = instance.state_from_named_values(initial_values)
```

Use `()` for scalar variables. Unknown keys raise `KeyError`; ambiguous
name/subscript pairs raise `ValueError`.

## SOS1 hints

`use_sos1="auto"` (the default) uses native SCIP SOS1 constraints when the
hint's ordinary formulation can be safely replaced. The adapter checks the
cardinality and link equations, variable bounds, and that auxiliary selectors
are not used by the objective or other active constraints. If a hint cannot be
validated, its ordinary constraints are kept.

Use `use_sos1="forced"` to require valid SOS1 hints; missing or unsupported hints
raise `OMMXPySCIPOptAdapterError` with the reason. `use_sos1="disabled"` solves
the ordinary formulation without consuming hints.

When a formulation is replaced, link-implied bounds are preserved in SCIP and
auxiliary selector values are restored by both `decode_to_state()` and
`decode()`. The original OMMX instance is unchanged, and the returned solution
is evaluated against its original constraints. Binary members used as their
own selectors remain solver variables.

If SCIP's numerical feasibility tolerance permits a state that cannot satisfy
the original rows after restoration, decoding raises
`OMMXPySCIPOptAdapterError`. Tighten SCIP's feasibility tolerance or use
`use_sos1="disabled"` to retain the original rows in the solver.

## Reference

TBW
