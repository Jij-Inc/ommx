---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
    jupytext_version: 1.19.1
kernelspec:
  display_name: ommx-update-books (3.9.23)
  language: python
  name: python3
---

Solve with Multiple Adapters and Compare the Results
=====================================================

OMMX Adapters share a common API, so you can solve the same problem with multiple solvers and compare their results. In [Solve Special Constraints Directly with the PySCIPOpt Adapter](./solve_special_constraints_with_pyscipopt_adapter.md) and [Prepare an Instance for an Adapter](./prepare_instance_for_adapter.md), we saw that different Adapters may accept different Instances directly. On this page, we use a simple model that both HiGHS and PySCIPOpt accept without conversion.

Each Adapter describes the inputs it accepts directly without conversion through its `INPUT_CLASS`. You can reuse the same `Instance` for a comparison only when it belongs to every Adapter's `INPUT_CLASS`. Otherwise, make a copy and prepare it separately for each Adapter.

Consider the following knapsack problem, which both HiGHS and SCIP accept directly:

$$
\begin{aligned}
\mathrm{maximize} \quad & \sum_{i=0}^{N-1} v_i x_i \\
\mathrm{s.t.} \quad & \sum_{i=0}^{n-1} w_i x_i - W \leq 0, \\
& x_{i} \in \{ 0, 1\}
\end{aligned}
$$

```{code-cell} ipython3
from ommx import Instance

v = [10, 13, 18, 31, 7, 15]
w = [11, 25, 20, 35, 10, 33]
W = 47
N = len(v)

instance = Instance.maximize()
x = [instance.new_binary("x", subscripts=[i]) for i in range(N)]
instance.objective = sum(v[i] * x[i] for i in range(N))
instance.add_constraint(
    sum(w[i] * x[i] for i in range(N)) <= W,
    "weight limit",
)
```

## Solve with Multiple Solvers

Here we use the open-source HiGHS and SCIP Adapters developed alongside the OMMX Python SDK. Adapters for other solvers use the same interface. See [Supported Adapters](../user_guide/supported_ommx_adapters.md) for the complete list.

```{code-cell} ipython3
from ommx_highs_adapter import OMMXHighsAdapter
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter


# Adapters to compare
adapters = {
    "highs": OMMXHighsAdapter,
    "scip": OMMXPySCIPOptAdapter,
}

# Solve the same exact input through each Adapter.
solutions = {
    name: adapter.solve(instance) for name, adapter in adapters.items()
}
```

## Compare the Results

This knapsack problem is small, so both solvers find an optimal solution.

```{code-cell} ipython3
from matplotlib import pyplot as plt

marks = {
    "highs": "o",
    "scip": "+",
}

for name, solution in solutions.items():
    x = solution.extract_decision_variables("x")
    subscripts = [key[0] for key in x.keys()]
    plt.plot(subscripts, x.values(), marks[name], label=name)

plt.legend()
```

For some analyses, it is useful to concatenate the pandas `DataFrame` values returned by `decision_variables_df()`.

```{code-cell} ipython3
import pandas

decision_variables = pandas.concat([
    solution.decision_variables_df().assign(solver=solver)
    for solver, solution in solutions.items()
])
decision_variables
```
