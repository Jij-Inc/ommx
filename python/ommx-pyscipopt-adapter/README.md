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
from ommx import DecisionVariable, Instance, Sense

x1 = DecisionVariable.integer(1, lower=0, upper=5)
ommx_instance = Instance.from_components(
    decision_variables=[x1],
    objective=x1,
    constraints={},
    sense=Sense.Minimize,
)

# Create `ommx.Solution` from the `pyscipot.Model`
ommx_solution = OMMXPySCIPOptAdapter.solve(ommx_instance)

print(ommx_solution)
```

## Initial solution by variable name

Resolve names and tuple subscripts against the instance you are solving, so
the same initial values can be reused when variable IDs change.

```python markdown-code-runner
from ommx import Instance, NamedState
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

instance = Instance.maximize()
x = instance.new_integer("x", lower=0, upper=5, subscripts=[0])
y = instance.new_integer("x", lower=0, upper=5, subscripts=[1])
instance.objective = 2 * x + y
instance.add_constraint(x + y <= 5)

initial_values: NamedState = {"x": {(0,): 1.0, (1,): 4.0}}
solution = OMMXPySCIPOptAdapter.solve(instance, initial_state=initial_values)
assert solution.state.entries == {x.id: 5.0, y.id: 0.0}

# Explicit conversion is also available when you need an ommx.State.
state = instance.state_from_named_values(initial_values)
```

For scalar variables use `()` as the subscript key. The conversion raises
`KeyError` for unknown names/subscripts and `ValueError` for ambiguous pairs.

## Reference

TBW
