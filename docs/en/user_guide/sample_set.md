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

ommx.v1.SampleSet
=================

[`ommx.v1.Solution`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Solution) represents a single solution returned by a solver. However, some solvers, often called samplers, can return multiple solutions. To accommodate this, OMMX provides two data structures for representing multiple solutions:

| Data Structure  | Description |
|:---------------|:------------|
| [`ommx.v1.Samples`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/sample_set_pb2/index.html#ommx.v1.sample_set_pb2.Samples) | A list of multiple solutions for decision variable IDs |
| [`ommx.v1.SampleSet`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.SampleSet) | Evaluations of objective and constraints with decision variables |

`Samples` corresponds to `State` and `SampleSet` corresponds to `Solution`. This notebook explains how to use `SampleSet`.

Creating a SampleSet
--------------------

Let's consider a simple optimization problem：

$$
\begin{aligned}
    \max &\quad x_1 + 2 x_2 + 3 x_3 \\
    \text{s.t.} &\quad x_1 + x_2 + x_3 = 1 \\
    &\quad x_1, x_2, x_3 \in \{0, 1\}
\end{aligned}
$$

```{code-cell} ipython3
from ommx.v1 import DecisionVariable, Instance

x = [DecisionVariable.binary(i) for i in range(3)]

instance = Instance.from_components(
    decision_variables=x,
    objective=x[0] + 2*x[1] + 3*x[2],
    constraints={0: sum(x) == 1},
    sense=Instance.MAXIMIZE,
)
```

Normally, solutions are provided by a solver, commonly referred to as a sampler, but for simplicity, we prepare them manually here. `ommx.v1.Samples` can hold multiple samples, each expressed as a set of values associated with decision variable IDs, similar to `ommx.v1.State`.

Each sample is assigned an ID. Some samplers issue their own IDs for logging, so OMMX allows specifying sample IDs. If omitted, IDs are assigned sequentially starting from `0`.

```{code-cell} ipython3
from ommx.v1 import Samples

# When specifying Sample ID
samples = Samples({
    0: {0: 1, 1: 0, 2: 0},  # x1 = 1, x2 = x3 = 0
    1: {0: 0, 1: 0, 2: 1},  # x3 = 1, x1 = x2 = 0
    2: {0: 1, 1: 1, 2: 0},  # x1 = x2 = 1, x3 = 0 (infeasible)
})# ^ sample ID
assert isinstance(samples, Samples)

# When automatically assigning Sample ID
samples = Samples([
    {0: 1, 1: 0, 2: 0},  # x1 = 1, x2 = x3 = 0
    {0: 0, 1: 0, 2: 1},  # x3 = 1, x1 = x2 = 0
    {0: 1, 1: 1, 2: 0},  # x1 = x2 = 1, x3 = 0 (infeasible)
])
assert isinstance(samples, Samples)
```

While `ommx.v1.Solution` is obtained via `Instance.evaluate`, `ommx.v1.SampleSet` can be obtained via `Instance.evaluate_samples`.

```{code-cell} ipython3
sample_set = instance.evaluate_samples(samples)
sample_set.summary
```

The `summary` attribute displays each sample's objective value and feasibility in a DataFrame format. For example, the sample with `sample_id=2` is infeasible and shows `feasible=False`. The table is sorted with feasible samples appearing first, and within them, those with better bjective values (depending on whether `Instance.sense` is maximization or minimization) appear at the top.

```{note}
For clarity, we explicitly pass `ommx.v1.Samples` created by `to_samples` to `evaluate_samples`, but you can omit it because `to_samples` would be called automatically.
```

Extracting individual samples
----------------------------
You can use `SampleSet.get` to retrieve each sample as an `ommx.v1.Solution` by specifying the sample ID:

```{code-cell} ipython3
from ommx.v1 import Solution

solution = sample_set.get(sample_id=0)
assert isinstance(solution, Solution)

print(f"{solution.objective=}")
solution.decision_variables_df()
```

Retrieving the best solution
---------------------------
`SampleSet.best_feasible` returns the best feasible sample, meaning the one with the highest objective value among all feasible samples:

```{code-cell} ipython3
solution = sample_set.best_feasible

print(f"{solution.objective=}")
solution.decision_variables_df()
```

Of course, if the problem is a minimization, the sample with the smallest objective value will be returned. If no feasible samples exist, an error will be raised.

```{code-cell} ipython3
sample_set_infeasible = instance.evaluate_samples([
    {0: 1, 1: 1, 2: 0},  # Infeasible since x0 + x1 + x2 = 2
    {0: 1, 1: 0, 2: 1},  # Infeasible since x0 + x1 + x2 = 2
])

# Every samples are infeasible
display(sample_set_infeasible.summary)

try:
    sample_set_infeasible.best_feasible
    assert False # best_feasible should raise RuntimeError
except RuntimeError as e:
    print(e)
```

```{note}
OMMX does not provide a method to determine which infeasible solution is the best, as many different criteria can be considered. Implement it yourself if needed.
```
