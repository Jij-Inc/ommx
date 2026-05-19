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

# Advanced Example: Annealing-Assisted Column Generation

This page shows how to express the pricing subproblem used in column generation as an
`ommx.v1.Instance`, then sample it with the OpenJij adapter.

The formulation follows the idea of
[Annealing-Assisted Column Generation for Inequality-Constrained Combinatorial Optimization Problems](https://arxiv.org/abs/2406.01887v1):
solve the restricted master problem with a conventional LP/MIP solver, use its dual
variables to build a smaller pricing subproblem, and send only that pricing subproblem
to an annealing sampler.

```{note}
This is an advanced modeling example, not a production CVRP solver. A full
implementation also needs route pool management, duplicate route filtering, LP dual
sign checks for the chosen master solver, and termination criteria.
```

## Column Generation Structure

For the capacitated vehicle routing problem (CVRP), column generation uses a route
pool $\Omega_\mathrm{RST}$ and repeatedly solves:

1. A restricted master problem (RMP), usually a linear relaxation of a set cover or
   set partitioning model.
2. A pricing subproblem, whose objective is the reduced cost
   $\omega_k - \sum_i a_{i,k} y_i - y_0$.
3. A final integer set cover or set partitioning model over the generated routes.

OMMX can represent both sides. The RMP is a linear `Instance` suitable for adapters
such as HiGHS, while the pricing subproblem is a binary constrained `Instance`
suitable for `OMMXOpenJijSAAdapter`.

The rest of this page focuses on one pricing solve, assuming that the current RMP
has already produced customer dual values $y_i$ and the vehicle-count dual value
$y_0$.

## CVRP Data

We use node `0` as the depot. Other nodes are customers with positive demand.

```{code-cell} ipython3
from dataclasses import dataclass
from math import hypot
from typing import Optional


@dataclass(frozen=True)
class CVRPData:
    points: list[tuple[float, float]]
    demand: list[int]
    capacity: int
    max_steps: int

    @property
    def node_count(self) -> int:
        return len(self.points)

    @property
    def distance(self) -> list[list[float]]:
        return [
            [hypot(x1 - x2, y1 - y2) for x2, y2 in self.points]
            for x1, y1 in self.points
        ]


data = CVRPData(
    points=[
        (0.0, 0.0),  # depot
        (1.0, 2.0),
        (2.0, 1.0),
        (3.0, 3.0),
        (4.0, 1.0),
        (5.0, 2.0),
    ],
    demand=[0, 2, 2, 3, 2, 1],
    capacity=5,
    max_steps=4,
)
```

Here is a synthetic set of dual values, standing in for the output of the current
restricted master problem. Larger customer duals make the pricing problem more likely
to include those customers.

```{code-cell} ipython3
customer_duals = {
    1: 5.0,
    2: 5.2,
    3: 6.0,
    4: 5.3,
    5: 4.8,
}
vehicle_dual = -0.5
```

## Build the Pricing Subproblem

Let $q_{t,i}$ be a binary variable that is `1` when node $i$ is selected at route
position $t$. The depot may appear in a position as padding. The route decoder below
removes intermediate depot visits and then adds the depot at both ends.

Compared with the paper notation, this example makes the depot legs explicit:

$$
\sum_i c_{0,i}q_{0,i}
+ \sum_{t=0}^{T-2}\sum_{i,j}c_{i,j}q_{t,i}q_{t+1,j}
+ \sum_i c_{i,0}q_{T-1,i}.
$$

The reduced-cost reward is

$$
- \sum_{i \ne 0} y_i \sum_t q_{t,i} - y_0.
$$

```{code-cell} ipython3
from ommx.v1 import DecisionVariable, Instance


def q_variable_id(t: int, i: int, node_count: int) -> int:
    return 1 + t * node_count + i


def build_pricing_instance(
    data: CVRPData,
    customer_duals: dict[int, float],
    vehicle_dual: float,
    *,
    forbidden_customers: Optional[set[int]] = None,
) -> Instance:
    forbidden_customers = forbidden_customers or set()
    node_count = data.node_count
    steps = data.max_steps
    distance = data.distance

    q = [
        [
            DecisionVariable.binary(
                q_variable_id(t, i, node_count),
                name="q",
                subscripts=[t, i],
            )
            for i in range(node_count)
        ]
        for t in range(steps)
    ]

    objective = -vehicle_dual

    # Cost from the depot to the first selected row, between rows, and back to the depot.
    objective += sum(distance[0][i] * q[0][i] for i in range(node_count))
    objective += sum(
        distance[i][j] * q[t][i] * q[t + 1][j]
        for t in range(steps - 1)
        for i in range(node_count)
        for j in range(node_count)
    )
    objective += sum(distance[i][0] * q[steps - 1][i] for i in range(node_count))

    # Dual reward for customers included in the route.
    objective -= sum(
        customer_duals[i] * q[t][i]
        for t in range(steps)
        for i in range(1, node_count)
    )

    constraints = {}
    cid = 0

    # Exactly one node is selected at each route position.
    for t in range(steps):
        constraints[cid] = (
            sum(q[t][i] for i in range(node_count)) == 1
        ).add_name("one-node-per-step").add_subscripts([t])
        cid += 1

    # A customer can appear at most once in the route.
    for i in range(1, node_count):
        constraints[cid] = (
            sum(q[t][i] for t in range(steps)) <= 1
        ).add_name("visit-at-most-once").add_subscripts([i])
        cid += 1

    # Capacity constraint for the route.
    constraints[cid] = (
        sum(data.demand[i] * q[t][i] for t in range(steps) for i in range(1, node_count))
        <= data.capacity
    ).add_name("capacity")
    cid += 1

    # Limited column generation can be represented by fixing q[t, i] = 0 for
    # customers that appeared in the previous pricing solution.
    for i in sorted(forbidden_customers):
        for t in range(steps):
            constraints[cid] = (
                q[t][i] == 0
            ).add_name("limited-column-fix").add_subscripts([t, i])
            cid += 1

    return Instance.from_components(
        decision_variables=[q[t][i] for t in range(steps) for i in range(node_count)],
        objective=objective,
        constraints=constraints,
        sense=Instance.MINIMIZE,
    )


pricing_instance = build_pricing_instance(data, customer_duals, vehicle_dual)
pricing_instance
```

The pricing instance still has constraints. The OpenJij adapter handles the conversion
pipeline: inequality constraints receive integer slack variables, constraints are
converted to penalty terms, integer variables are log-encoded, and the final QUBO/HUBO
is sampled.

## Sample with OpenJij

The penalty coefficient controls the tradeoff between the reduced-cost objective and
constraint satisfaction. A practical implementation should tune this value and inspect
the returned feasibility flags. For this small example, a simple scale based on the
largest distance or dual coefficient is enough.

```{code-cell} ipython3
from ommx_openjij_adapter import OMMXOpenJijSAAdapter


distance_scale = max(
    data.distance[i][j]
    for i in range(data.node_count)
    for j in range(data.node_count)
    if i != j
)
dual_scale = max(abs(value) for value in customer_duals.values())
penalty_weight = 2.0 * max(1.0, distance_scale, dual_scale)

sample_set = OMMXOpenJijSAAdapter.sample(
    pricing_instance,
    num_reads=64,
    num_sweeps=2000,
    uniform_penalty_weight=penalty_weight,
    seed=0,
)
sample_set.summary.head()
```

## Decode a Route

OpenJij returns values for the binary variables. OMMX evaluates the samples against the
original constrained pricing instance, so `sample_set.summary` can be used to filter
for feasible samples before decoding the route.

```{code-cell} ipython3
def route_from_sample(sample: dict[tuple[int, ...], float]) -> list[int]:
    selected = []
    for t in range(data.max_steps):
        row = {i: sample[(t, i)] for i in range(data.node_count)}
        node = max(row, key=row.get)
        if node != 0:
            selected.append(node)
    return [0, *selected, 0]


def route_cost(route: list[int], distance: list[list[float]]) -> float:
    return sum(distance[i][j] for i, j in zip(route, route[1:]))


def reduced_cost(
    route: list[int],
    data: CVRPData,
    customer_duals: dict[int, float],
    vehicle_dual: float,
) -> float:
    customers = set(route) - {0}
    return (
        route_cost(route, data.distance)
        - sum(customer_duals[i] for i in customers)
        - vehicle_dual
    )


feasible_ids = sample_set.summary.query("feasible == True").index
if len(feasible_ids) == 0:
    raise RuntimeError("OpenJij returned no feasible pricing samples")

sample_id = feasible_ids[0]
sample = sample_set.extract_decision_variables("q", sample_id)
route = route_from_sample(sample)
route, reduced_cost(route, data, customer_duals, vehicle_dual)
```

If the reduced cost is negative, the route is a candidate column for the restricted
master problem. In a full column generation loop, append that route to the route pool
and solve the RMP again.

## RMP Skeleton

The restricted master problem is a linear model over route variables $\theta_k$.
The LP relaxation uses continuous nonnegative variables; the final problem can use
binary variables and equality coverage constraints.

```python
from dataclasses import dataclass
from ommx.v1 import DecisionVariable, Instance
from ommx_highs_adapter import OMMXHighsAdapter


@dataclass(frozen=True)
class Route:
    nodes: tuple[int, ...]
    cost: float
    customers: frozenset[int]


def build_restricted_master(
    data: CVRPData,
    routes: list[Route],
    vehicle_count: int,
    *,
    relax: bool,
    partition: bool,
) -> Instance:
    theta = [
        DecisionVariable.continuous(k, lower=0.0) if relax
        else DecisionVariable.binary(k)
        for k in range(len(routes))
    ]

    objective = sum(route.cost * theta[k] for k, route in enumerate(routes))
    constraints = {}

    # Cover every customer. The LP RMP usually uses >= 1; the final set partition
    # model can use == 1 to remove overlap.
    for customer in range(1, data.node_count):
        covering_routes = [
            theta[k] for k, route in enumerate(routes)
            if customer in route.customers
        ]
        if partition:
            constraints[customer - 1] = sum(covering_routes) == 1
        else:
            constraints[customer - 1] = 1 - sum(covering_routes) <= 0

    constraints[data.node_count - 1] = sum(theta) == vehicle_count

    return Instance.from_components(
        decision_variables=theta,
        objective=objective,
        constraints=constraints,
        sense=Instance.MINIMIZE,
    )


rmp = build_restricted_master(data, routes, vehicle_count=2, relax=True, partition=False)
rmp_solution = OMMXHighsAdapter.solve(rmp)

# For constraints modeled as 1 - sum(theta) <= 0, the sign convention of the
# returned LP dual may need to be converted before it is used as y_i.
customer_duals = {}
for i in range(1, data.node_count):
    dual = rmp_solution.get_dual_variable(i - 1)
    if dual is None:
        raise RuntimeError(f"Missing dual variable for customer constraint {i}")
    customer_duals[i] = -dual

vehicle_dual = rmp_solution.get_dual_variable(data.node_count - 1)
if vehicle_dual is None:
    raise RuntimeError("Missing dual variable for the vehicle-count constraint")
```

The complete control flow is then:

```python
routes = initial_feasible_routes(data)

for _ in range(max_iterations):
    rmp = build_restricted_master(data, routes, vehicle_count, relax=True, partition=False)
    rmp_solution = OMMXHighsAdapter.solve(rmp)

    customer_duals, vehicle_dual = extract_duals(rmp_solution)
    pricing = build_pricing_instance(data, customer_duals, vehicle_dual)
    samples = OMMXOpenJijSAAdapter.sample(pricing, uniform_penalty_weight=penalty_weight)

    route = decode_best_negative_reduced_cost_route(samples)
    if route is None:
        break
    routes.append(route)

final = build_restricted_master(data, routes, vehicle_count, relax=False, partition=True)
solution = OMMXHighsAdapter.solve(final)
```

The key modeling point is that OMMX keeps the pricing model as a constrained
optimization problem. The OpenJij adapter is responsible for converting it to QUBO or
HUBO, and the returned `SampleSet` is evaluated against the constrained OMMX instance
used to build the pricing problem.
