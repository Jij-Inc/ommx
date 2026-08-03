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
The TSP instance above contains constraints, so obtain a common OMMX preparation
Policy with an explicit finite penalty weight. Apply that Policy to the
`Instance`, pass the resulting `preparation.input` to the Adapter, and evaluate
the decoded states against the retained source explicitly.

```{code-cell} ipython3
from ommx_openjij_adapter import OMMXOpenJijSAAdapter

policy = OMMXOpenJijSAAdapter.recommended_preparation_policy(
    uniform_penalty_weight=20.0,
)
preparation = instance.prepare(policy)

input_sample_set = OMMXOpenJijSAAdapter.sample(
    preparation.input,
    num_reads=16,
)
source_samples = preparation.decode(input_sample_set.samples)
sample_set = preparation.source.evaluate_samples(
    source_samples,
    atol=preparation.policy.atol,
)
sample_set.summary
```

{py:meth}`~ommx_openjij_adapter.OMMXOpenJijSAAdapter.sample` returns
{py:class}`~ommx.SampleSet`, which stores the evaluated objective values and
constraint violations in addition to the decision variable values.
`SampleSet.summary` displays this information. Its `feasible` column indicates
feasibility for the source constrained problem because the final
`evaluate_samples()` call evaluates the decoded states against that source
model.

The penalty weight in `policy` belongs to the explicit preparation, not to the
OpenJij backend sampler. A finite penalty encourages feasibility but does not
guarantee that every returned sample is feasible for the source problem.

### Inspecting preparation

`Instance.prepare()` leaves `instance` unchanged. The returned value retains
isolated source, Policy, and input snapshots together with the applied
Transforms. It records construction; it does not copy solver statuses or claim
that finite penalties preserve source feasibility or optimality.

```{code-cell} ipython3
OMMXOpenJijSAAdapter.require_applicable(preparation.input)
{
    "target_membership": preparation.policy.acceptable_instance_class.contains(
        preparation.input
    ),
    "transforms": [transform.name for transform in preparation.transforms],
}
```

Discrete integer slack approximation requires passing
`allow_approximate_integer_slack=True` when obtaining the Policy; setting only
`inequality_integer_slack_max_range` does not opt into approximation.
`uniform_penalty_weight` and `penalty_weights` explicitly select a finite
penalty Transform. Per-constraint weights address regular constraint IDs; when
special-constraint lowering adds regular constraints, use a uniform weight.

If variable bounds prove an inequality infeasible, `Instance.prepare()` raises
the core-owned {py:class}`~ommx.InfeasibleDetected`; the
historical {py:class}`~ommx.adapter.InfeasibleDetected` import is an alias for
the same exception object. This is a property of the model, not an adapter
applicability failure.

The maximum of 53 auxiliary bits checked for a used Integer variable is an
availability limit of OMMX Integer-to-Binary log encoding. It is not a property of the
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
