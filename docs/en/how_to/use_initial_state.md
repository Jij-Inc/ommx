---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
    jupytext_version: 1.19.1
kernelspec:
  display_name: Python 3 (ipykernel)
  language: python
  name: python3
---

# Supply an initial candidate to an Adapter

Use an earlier result or a supplied candidate as a starting point for search. This page constructs a small problem and initial candidate using the [SDK and PySCIPOpt Adapter](../getting_started/install.md), without another tutorial's runtime state.

## Evaluate the candidate before supplying it

`initial_state` accepts a dictionary from variable IDs to values, or a {py:class}`~ommx.State`. It proposes a candidate to the backend; it does not specify the final result.

```{code-cell} ipython3
from ommx import Instance
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

instance = Instance.maximize()
x = instance.new_integer("x", lower=0, upper=5)
y = instance.new_integer("y", lower=0, upper=5)
instance.objective = x - y
instance.add_constraint(x + y <= 5)

initial_state = {x.id: 3.0, y.id: 2.0}
initial = instance.evaluate(initial_state)
assert initial.feasible
assert initial.objective == 1

solution = OMMXPySCIPOptAdapter.solve(instance, initial_state=initial_state)
assert solution.feasible
assert solution.objective == 5
```

The initial objective is 1 and the returned objective is 5. Read the returned values through Solution. An initial candidate does not necessarily speed up search; this is not a timing comparison.

Support for initial candidates and omitted values varies between Adapters. Check the selected Adapter's `initial_state` contract. In particular, a partial candidate may not contain every value required by `Instance.evaluate()`.

## Supply a State

```{code-cell} ipython3
from ommx import State

state = State(entries=initial_state)
solution = OMMXPySCIPOptAdapter.solve(instance, initial_state=state)
assert solution.feasible
```

(named-initial-state)=
## Match values by names and subscripts

```{note}
This section describes development functionality. Public `3.0.0b7` does not have `state_from_named_values()`; use the ID-based procedure above. You do not need to upgrade the b7 environment for named input.
```

If rebuilding a model changes IDs, use {py:meth}`~ommx.Instance.state_from_named_values` to construct a State from names and subscripts. Match the SDK/Adapter development revision to the [API Reference](../reference/index.md). Use `()` for scalar subscripts and `(i,)` for one dimension.

```python
named_values = {"x": {(): 3.0}, "y": {(): 2.0}}
state = instance.state_from_named_values(named_values)
solution = OMMXPySCIPOptAdapter.solve(instance, initial_state=state)
```

Use IDs when several variables match the same name and subscripts. A missing match is not silently assigned to another variable. See {py:meth}`~ommx.Instance.state_from_named_values` for exact matching rules and errors.

The same procedure applies to larger problems: match the candidate's source to the target Instance and evaluate it when possible before supplying it. Instead of pasting a large dictionary, consider loading a candidate file or reusing {py:attr}`~ommx.Solution.state`.
