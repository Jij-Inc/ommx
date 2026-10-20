# Initial Values by Name and Subscripts

SCIP accepts initial values grouped by variable name and tuple subscripts.
The adapter resolves them against the instance being solved, so the same
input can be reused when rebuilding a model changes variable IDs.

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
```

To obtain a `State` explicitly, use `Instance.state_from_named_values()`:

```python markdown-code-runner
state = instance.state_from_named_values(initial_values)
assert state.entries == {x.id: 1.0, y.id: 4.0}
```

Use tuple keys: `()` for scalars, `(i,)` for one index, and `(i, j)` for two.
The output of `Solution.extract_all_decision_variables()` can also be converted
against another instance's IDs.

Unknown name/subscript pairs raise `KeyError`; ambiguous pairs raise `ValueError`.
Parameters and descriptions do not distinguish keys. For ambiguous variables,
use an ID-keyed state. Partial mappings are accepted and omitted values are not
filled; their treatment as initial values depends on the adapter and solver.
Conversion does not check finiteness, bounds, kind, fixed values, or dependencies.
Use evaluation for those checks. Neither the instance nor the input is modified.
