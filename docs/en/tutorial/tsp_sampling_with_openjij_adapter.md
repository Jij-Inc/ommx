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

# Sampling from QUBO with OMMX Adapter

Here, we explain how to convert a problem to QUBO and perform sampling using the Traveling Salesman Problem as an example.

```{figure} ./assets/taraimawashi_businessman.png
[Illustration of a man in a suit](https://www.irasutoya.com/2017/03/blog-post_739.html)
```

The Traveling Salesman Problem (TSP) is about finding a route for a salesman to visit multiple cities in sequence. Given the travel costs between cities, we seek to find the path that minimizes the total cost. For this self-contained example, we reproducibly generate 16 cities in a 10 by 10 region using a fixed random seed:

```{code-cell} ipython3
from random import Random

N = 16
rng = Random(42)
city_points = [
    (rng.uniform(0.0, 10.0), rng.uniform(0.0, 10.0))
    for _ in range(N)
]
```

Let's plot the locations of the cities.

```{code-cell} ipython3
%matplotlib inline
from matplotlib import pyplot as plt

x_coords, y_coords = zip(*city_points)
plt.scatter(x_coords, y_coords)
plt.xlabel('X Coordinate')
plt.ylabel('Y Coordinate')
plt.title('Randomly Generated City Locations')
plt.show()
```

Let's consider distance as the cost. We'll calculate the distance $d(i, j)$ between city $i$ and city $j$.

```{code-cell} ipython3
def distance(x, y):
    return ((x[0] - y[0])**2 + (x[1] - y[1])**2)**0.5

# Distance between each pair of cities
d = [[distance(city_points[i], city_points[j]) for i in range(N)] for j in range(N)]
```

Using this, we can formulate TSP as follows. First, let's represent whether we are at city $i$ at time $t$ with a binary variable $x_{t, i}$. Then, we seek $x_{t, i}$ that satisfies the following constraints. The distance traveled by the salesman is given by:

$$
\sum_{t=0}^{N-1} \sum_{i, j = 0}^{N-1} d(i, j) x_{t, i} x_{(t+1 \% N), j}
$$

However, $x_{t, i}$ cannot be chosen freely and must satisfy two constraints: at each time $t$, the salesman can only be in one city, and each city must be visited exactly once:

$$
\sum_{i=0}^{N-1} x_{t, i} = 1, \quad \sum_{t=0}^{N-1} x_{t, i} = 1
$$

Combining these, TSP can be formulated as a constrained optimization problem:

$$
\begin{aligned}
\min \quad & \sum_{t=0}^{N-1} \sum_{i, j = 0}^{N-1} d(i, j) x_{t, i} x_{(t+1 \% N), j} \\
\text{s.t.} \quad & \sum_{i=0}^{N-1} x_{t, i} = 1 \quad (\forall t = 0, \ldots, N-1) \\
\quad & \sum_{t=0}^{N-1} x_{t, i} = 1 \quad (\forall i = 0, \ldots, N-1)
\end{aligned}
$$

The corresponding `ommx.Instance` can be created as follows:

```{code-cell} ipython3
from ommx import DecisionVariable, Instance

x = [[
        DecisionVariable.binary(
            i + N * t,  # Decision variable ID
            name="x",           # Name of the decision variable, used when extracting solutions
            subscripts=[t, i])  # Subscripts of the decision variable, used when extracting solutions
        for i in range(N)
    ]
    for t in range(N)
]

objective = sum(
    d[i][j] * x[t][i] * x[(t+1) % N][j]
    for i in range(N)
    for j in range(N)
    for t in range(N)
)
place_constraint = {
    t: (sum(x[t][i] for i in range(N)) == 1)
        .set_name("place")
        .add_subscripts([t])
    for t in range(N)
}
time_constraint = {
    i + N: (sum(x[t][i] for t in range(N)) == 1)
        .set_name("time")
        .add_subscripts([i])
    for i in range(N)
}

instance = Instance.from_components(
    decision_variables=[x[t][i] for i in range(N) for t in range(N)],
    objective=objective,
    constraints={**place_constraint, **time_constraint},
    sense=Instance.MINIMIZE
)
```

The variable names and subscripts added to `DecisionVariable.binary` during creation will be used later when interpreting the obtained samples.

+++


## Sampling with OpenJij

The OpenJij adapter's input class contains Binary, unconstrained minimization
instances with a polynomial objective of any degree.
The TSP instance above contains constraints, so prepare it explicitly with a
finite penalty weight. Then pass the resulting `prepared.input` `Instance` to
the Adapter and evaluate those samples against the source model explicitly.

```{code-cell} ipython3
from ommx_openjij_adapter import OMMXOpenJijSAAdapter

prepared = OMMXOpenJijSAAdapter.prepare(
    instance,
    uniform_penalty_weight=20.0,
)

prepared_samples = OMMXOpenJijSAAdapter.sample(
    prepared.input,
    num_reads=16,
)
sample_set = prepared.evaluate_source(prepared_samples)
sample_set.summary
```

{py:meth}`~ommx_openjij_adapter.OMMXOpenJijSAAdapter.sample` returns
{py:class}`~ommx.SampleSet`, which stores the evaluated objective values and
constraint violations in addition to the decision variable values.
`SampleSet.summary` displays this information. Its `feasible` column indicates
feasibility for the source constrained problem because
`prepared.evaluate_source()` evaluates the prepared-input states against that
source model.

The penalty weight passed to `prepare` belongs to the explicit preparation, not
to the OpenJij backend sampler. A finite penalty encourages feasibility but does
not guarantee that every returned sample is feasible for the source problem.

### Inspecting preparation

`check_preparation` checks the source model and preparation options without
mutating the instance. `prepare` performs the checked transformations and
stores an audit report in `prepared.report`:

```{code-cell} ipython3
report = prepared.report
final = report.input_applicability
{
    "source_membership": report.source_check.source_membership.is_member,
    "preconditions": report.source_check.precondition_violations,
    "steps": [step.operation for step in report.steps],
    "input_applicability": final.is_applicable if final else False,
}
```

The report separates three questions:

- `source_check` records membership in the preparation source class and the
  Adapter-owned preparation preconditions.
- `steps` records each OpenJij-specific operation that was applied.
- `input_applicability` says whether `prepared.input` belongs to the Adapter
  input class and satisfies its Adapter-specific preconditions.

This step list is an operation audit, not a composed mathematical guarantee.
Common preparation policy, guarantees, and automatic selection are tracked in
[OMMX issue #1111](https://github.com/Jij-Inc/ommx/issues/1111). By default,
OpenJij preparation uses only the available exact operations. Discrete integer
slack approximation requires `allow_approximate_integer_slack=True`; selecting
an integer slack range alone does not opt into approximation. Supplying penalty
weights explicitly selects finite-penalty preparation, which does not claim
that the Adapter directly or exactly supports constrained input.

If variable bounds prove an inequality infeasible, `check_preparation` and
`prepare` raise {py:class}`~ommx.adapter.InfeasibleDetected`; that is a property
of the model, not an adapter applicability failure.

The maximum of 53 auxiliary bits checked for a used Integer variable is an
OMMX Integer-to-Binary log-encoding condition. It is not a property of the
OpenJij adapter's input class and is unrelated to `ommx.v2.Feature`, which gates
whether a reader can safely interpret serialized semantics for forward
compatibility. Spin-variable support, including direct Spin input for OpenJij,
is tracked separately in
[OMMX issue #1082](https://github.com/Jij-Inc/ommx/issues/1082).

To view the feasibility for each constraint, use the `summary_with_constraints` property.

```{code-cell} ipython3
sample_set.summary_with_constraints
```

For more detailed information, you can use the `SampleSet.decision_variables_df()` and `SampleSet.constraints_df()` methods.

```{code-cell} ipython3
sample_set.decision_variables_df().head(2)
```

```{code-cell} ipython3
sample_set.constraints_df().head(2)
```

To obtain the samples, use the `SampleSet.extract_decision_variables` method. This interprets the samples using the `name` and `subscripts` registered when creating `ommx.DecisionVariables`. For example, to get the value of the decision variable named `x` with `sample_id=1`, use the following to obtain it in the form of `dict[subscripts, value]`.

```{code-cell} ipython3
sample_id = 1
x = sample_set.extract_decision_variables("x", sample_id)
t = 2
i = 3
x[(t, i)]
```

Since we obtained a sample for $x_{t, i}$, we convert this into a TSP path. This depends on the formulation used, so you need to write the processing yourself.

```{code-cell} ipython3
def sample_to_path(sample: dict[tuple[int, ...], float]) -> list[int]:
    path = []
    for t in range(N):
        for i in range(N):
            if sample[(t, i)] == 1:
                path.append(i)
    return path
```

Let's display this. First, we obtain the IDs of samples that are feasible for the original problem.

```{code-cell} ipython3
feasible_ids = sample_set.summary.query("feasible == True").index
feasible_ids
```

Let's display the optimized paths for these samples.

```{code-cell} ipython3
fig, axie = plt.subplots(3, 3, figsize=(12, 12))

for i, ax in enumerate(axie.flatten()):
    if i >= len(feasible_ids):
        break
    s = feasible_ids[i]
    x = sample_set.extract_decision_variables("x", s)
    path = sample_to_path(x)
    xs = [city_points[i][0] for i in path] + [city_points[path[0]][0]]
    ys = [city_points[i][1] for i in path] + [city_points[path[0]][1]]
    ax.plot(xs, ys, marker='o')
    ax.set_title(f"Sample {s}, objective={sample_set.objectives[s]:.2f}")

plt.tight_layout()
plt.show()
```
