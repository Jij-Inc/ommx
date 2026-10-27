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

ommx.SampleSet
=================

[Evaluating a State to obtain a Solution](./solution.md) covers one candidate. To evaluate several candidates against the same Instance, use {class}`~ommx.Samples` and {class}`~ommx.SampleSet`. Candidates can come from a solver called a sampler or be supplied manually.

| Data structure | Meaning |
|:----------|:-----|
| {class}`~ommx.Samples` | A variable-ID-to-value assignment for each candidate |
| {class}`~ommx.SampleSet` | Candidate assignments together with evaluated objectives and constraints |

`Samples` corresponds to `State`, and `SampleSet` to `Solution`. Feasibility and objective values have the same meanings as in Solution.

Creating a SampleSet
--------------

Consider this small optimization problem:

$$
\begin{aligned}
    \max &\quad x_1 + 2 x_2 + 3 x_3 \\
    \text{s.t.} &\quad x_1 + x_2 + x_3 = 1 \\
    &\quad x_1, x_2, x_3 \in \{0, 1\}
\end{aligned}
$$

```{code-cell} ipython3
from ommx import DecisionVariable, Instance, Sense

x = [DecisionVariable.binary(i) for i in range(3)]

instance = Instance.from_components(
    decision_variables=x,
    objective=x[0] + 2*x[1] + 3*x[2],
    constraints={0: sum(x) == 1},
    sense=Sense.Maximize,
)
```

Candidates are usually obtained from a sampler, but here we supply them manually. As its name suggests, `ommx.Samples` holds multiple samples. Each sample assigns values to decision-variable IDs, just as `ommx.State` does.

Each sample also has an ID. Some samplers generate IDs for identifying log entries, so you can specify them explicitly. If omitted, IDs are assigned sequentially from `0`.

```{code-cell} ipython3
from ommx import Samples

# Sample IDを指定する場合
samples = Samples({
    0: {0: 1, 1: 0, 2: 0},  # x1 = 1, x2 = x3 = 0
    1: {0: 0, 1: 0, 2: 1},  # x3 = 1, x1 = x2 = 0
    2: {0: 1, 1: 1, 2: 0},  # x1 = x2 = 1, x3 = 0 (infeasible)
})# ^ sample ID
assert isinstance(samples, Samples)

# Sample IDを自動で割り振る場合
samples = Samples([
    {0: 1, 1: 0, 2: 0},  # x1 = 1, x2 = x3 = 0
    {0: 0, 1: 0, 2: 1},  # x3 = 1, x1 = x2 = 0
    {0: 1, 1: 1, 2: 0},  # x1 = x2 = 1, x3 = 0 (infeasible)
])
assert isinstance(samples, Samples)
```

Where `Instance.evaluate` produces an `ommx.Solution`, `Instance.evaluate_samples` produces an `ommx.SampleSet`.

```{code-cell} ipython3
sample_set = instance.evaluate_samples(samples)
sample_set.summary
```

The `summary` property displays objective values and feasibility as a DataFrame. Sample `sample_id=2` violates the constraint, so `feasible=False`. The table places feasible samples first, then orders objective values according to `SampleSet.sense`, with better values first.

```{note}
Instead of a `Samples` object, `evaluate_samples` also accepts a dictionary keyed by sample IDs or a list of assignments.
```

Retrieving an individual sample
---------------------

Use `SampleSet.get` to retrieve a sample by ID as an `ommx.Solution`:

```{code-cell} ipython3
from ommx import Solution

solution = sample_set.get(sample_id=0)
assert isinstance(solution, Solution)

print(f"{solution.objective=}")
solution.decision_variables_df()
```

Selecting the best feasible candidate obtained
-------------

`SampleSet.best_feasible` returns the feasible sample with the best objective according to `sense`. Being best among these candidates does not imply global optimality for the original problem.

```{code-cell} ipython3
solution = sample_set.best_feasible

print(f"{solution.objective=}")
solution.decision_variables_df()
```

For a minimization problem, it returns the smallest objective value. If no sample is feasible, it raises an error.

```{code-cell} ipython3
sample_set_infeasible = instance.evaluate_samples([
    {0: 1, 1: 1, 2: 0},  # Infeasible since x0 + x1 + x2 = 2
    {0: 1, 1: 0, 2: 1},  # Infeasible since x0 + x1 + x2 = 2
])

# Every samples are infeasible
display(sample_set_infeasible.summary)

try:
    sample_set_infeasible.best_feasible
    assert False # best_feasible should raise ValueError
except ValueError as e:
    print(e)
```

```{note}
There are many possible criteria for ranking infeasible candidates. OMMX does not choose one; implement the criterion needed for your task.
```

Evaluating a transformed problem
-------------------------

The examples above evaluate the problem as constructed. If you transform an Instance, evaluate candidates against the transformed Instance.

Removed constraints are conditions excluded from the current problem but retained for checking results. An Instance may also retain an output objective for reporting, separate from its current objective. Calling `evaluate_samples` on such an Instance uses that retained information in the SampleSet's evaluation results.

See [Transformed problems and removed constraints](./removed_constraints.md) for which constraints enter feasibility and how reported objective values differ from solver-facing ones. Here too, best among the obtained candidates and global optimality for the original problem are different claims.
