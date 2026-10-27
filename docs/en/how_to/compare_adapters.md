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

# Solve with several Adapters and compare results

Once you can solve a problem with an Adapter, pass the same Instance to several Adapters and compare their results. Use a small model that both HiGHS and PySCIPOpt accept without transformation. Follow [Install the Python SDK and Adapters](../getting_started/install.md) to install the v3 SDK, both Adapters, and matplotlib.

`solve()` does not modify the original Instance, so reuse one Instance. When Adapters need different transformations, each prepares an independent copy. See [Choose which constraint families Preparation transforms](./customize_preparation.md) for custom Policies.

Consider this knapsack problem:

$$
\begin{aligned}
\mathrm{maximize} \quad & \sum_{i=0}^{N-1} v_i x_i \\
\mathrm{s.t.} \quad & \sum_{i=0}^{N-1} w_i x_i - W \leq 0, \\
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
    "重量制限",
)
```

## Solve with several solvers

Use the open-source HiGHS and SCIP integrations developed alongside the SDK. Other solver Adapters use the same interface; see [Choose an OMMX Adapter](../user_guide/supported_ommx_adapters.md).

```{code-cell} ipython3
from ommx_highs_adapter import OMMXHighsAdapter
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter


# 利用するAdapterの一覧
adapters = {
    "highs": OMMXHighsAdapter,
    "scip": OMMXPySCIPOptAdapter,
}

# 各Adapterを介して問題を解く
solutions = {
    name: adapter.solve(instance) for name, adapter in adapters.items()
}
```

## Compare the results

First compare reported objective values and feasibility for the same Instance. Include `instance.sense` because better means larger for maximization and smaller for minimization.

```{code-cell} ipython3
import pandas

comparison = pandas.DataFrame([
    {
        "adapter": name,
        "sense": str(instance.sense),
        "objective": solution.objective,
        "feasible": solution.feasible,
        "optimality": str(solution.optimality),
    }
    for name, solution in solutions.items()
]).set_index("adapter")
comparison
```

These are OMMX-reported objective values. Feasibility includes variable domains and constraints. Read feasibility, agreement in objective values, and established optimality separately. This example demonstrates a comparison workflow, not that either solver is generally superior.

### Separate the comparison axes

| Information | Basis for comparison |
|---|---|
| Reported objective and sense | The same problem and objective meaning |
| Original-problem feasibility | The same evaluation target and tolerance |
| Optimality | The target problem and the guarantee represented by the Adapter |
| Backend bound, gap, and termination | The formulation actually solved, its objective, and backend-specific definitions |
| Execution time | Separate measurements with defined data, settings, environment, and repetitions |

When explicitly solving different transformed Instances, evaluate candidates against the original Instance too when needed. Value recovery or transformed class membership alone does not guarantee original feasibility or global optimality. Consult [Transformation guarantees](../user_guide/transformation_guarantees.md) and [Transformed results](../user_guide/removed_constraints.md) to identify the evaluation target.

To inspect bounds and gaps, use [Adapter diagnostics](inspect_adapter_diagnostics.md). In particular, do not interpret a backend bound as a bound on the original objective without accounting for penalties or objective-sign changes. Experiment is not a prerequisite for this comparison.

### Plot candidate variable values

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

For some analyses it is convenient to concatenate the pandas DataFrames returned by `decision_variables_df()`:

```{code-cell} ipython3
import pandas

decision_variables = pandas.concat([
    solution.decision_variables_df().assign(solver=solver)
    for solver, solution in solutions.items()
])
decision_variables
```
