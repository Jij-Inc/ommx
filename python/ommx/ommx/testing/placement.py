r"""Plant Placement Problem — equivalent OMMX formulations.

This module provides a small, solver-agnostic benchmark problem used to
exercise an adapter's SOS1 handling. The builders in this module produce
`ommx.v1.Instance` objects describing the same feasible region and optimum;
they differ only in how "at most one plant per region" is communicated to
the solver.

Problem
-------

A set of plants and clients are drawn uniformly from :math:`[0, 100]^2`. A
vertical line at :math:`x = 50` partitions the plants into a *west* region
:math:`W = \{i : x_i^{\text{plant}} < 50\}` and an *east* region
:math:`E = \{i : x_i^{\text{plant}} \ge 50\}`. At most one plant may be
opened in each region; the opened plant covers all of its region's share of
client demand via a continuous transport variable.

Sets and parameters
~~~~~~~~~~~~~~~~~~~

- :math:`N` plants, :math:`M` clients.
- :math:`C_i \ge 0` — maximum capacity of plant :math:`i`.
- :math:`d_j \ge 0` — demand of client :math:`j`.
- :math:`\operatorname{dist}(i, j)` — Euclidean distance between plant
  :math:`i` and client :math:`j`.

Decision variables (shared by both formulations)
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. math::

    \begin{aligned}
    s_{i,j} &\in [0, d_j] \qquad i \in 1..N,\ j \in 1..M \\
    c_i     &\in [0, C_i] \qquad i \in 1..N
    \end{aligned}

where :math:`s_{i,j}` is the amount delivered from plant :math:`i` to client
:math:`j` and :math:`c_i` is the total capacity drawn from plant :math:`i`.

Shared constraints
~~~~~~~~~~~~~~~~~~

.. math::

    \begin{aligned}
    \sum_{j=1}^M s_{i,j} &= c_i \quad \text{(capacity balance, per plant)} \\
    \sum_{i=1}^N s_{i,j} &= d_j \quad \text{(demand, per client)}
    \end{aligned}

Objective (minimize)
~~~~~~~~~~~~~~~~~~~~

.. math::

    \min \; \sum_{i,j} \operatorname{dist}(i, j) \cdot s_{i,j}
          \;+\; \sum_i c_i

"At most one plant per region" — eight formulations
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

All eight builders share the decision variables and constraints above and
encode the same feasible region on :math:`(s, c)`. They differ along three
orthogonal axes:

- whether the auxiliary opening indicator :math:`\delta_i \in \{0, 1\}` and
  the big-M link :math:`c_i \le C_i \, \delta_i` are introduced;
- whether the per-region cardinality :math:`\sum_{i \in W} \delta_i \le 1`
  (and the analogous east bound) is added as a plain linear constraint;
- where SOS1 is declared: on the continuous capacities
  :math:`\{c_i\}_{i \in W/E}`, on the binary indicators
  :math:`\{\delta_i\}_{i \in W/E}`, on **both** (redundant), or nowhere.

The eight builders enumerate every well-defined combination:

+---+-----------------------------------------+------------+---------------------+--------------+--------------+
| # | Builder                                 | δ + big-M  | :math:`\sum δ ≤ 1`  | SOS1 on `c`  | SOS1 on `δ`  |
+===+=========================================+============+=====================+==============+==============+
| 1 | :func:`build_sos1`                      | –          | –                   | ✓            | –            |
+---+-----------------------------------------+------------+---------------------+--------------+--------------+
| 2 | :func:`build_sos1_on_c_with_delta`      | ✓          | –                   | ✓            | –            |
+---+-----------------------------------------+------------+---------------------+--------------+--------------+
| 3 | :func:`build_sos1_on_c_with_delta_with_card` | ✓     | ✓                   | ✓            | –            |
+---+-----------------------------------------+------------+---------------------+--------------+--------------+
| 4 | :func:`build_sos1_on_delta`             | ✓          | –                   | –            | ✓            |
+---+-----------------------------------------+------------+---------------------+--------------+--------------+
| 5 | :func:`build_sos1_on_delta_with_card`   | ✓          | ✓                   | –            | ✓            |
+---+-----------------------------------------+------------+---------------------+--------------+--------------+
| 6 | :func:`build_sos1_on_both_with_delta`   | ✓          | –                   | ✓            | ✓            |
+---+-----------------------------------------+------------+---------------------+--------------+--------------+
| 7 | :func:`build_sos1_on_both_with_delta_with_card` | ✓  | ✓                   | ✓            | ✓            |
+---+-----------------------------------------+------------+---------------------+--------------+--------------+
| 8 | :func:`build_bigm`                      | ✓          | ✓                   | –            | –            |
+---+-----------------------------------------+------------+---------------------+--------------+--------------+

The δ-bearing rows always include the big-M link
:math:`c_i \le C_i \, \delta_i`. Each region with at least two plants
contributes one SOS1 set per "✓" in the SOS1 columns; regions with fewer
than two plants are skipped (the constraint is trivially satisfied).

Intended use
------------

These eight builders are useful for benchmarking how a solver — and the
adapter forwarding to it — reacts to different ways of expressing the same
SOS1 structure. Callers should construct :meth:`Input.random` with a fixed
``random.seed`` for reproducibility and pass the resulting :class:`Input`
to each builder to obtain comparable instances.
"""

from __future__ import annotations

from dataclasses import dataclass
from math import ceil, sqrt
import random
from typing import Dict, List, Tuple

from ommx.v1 import DecisionVariable, Instance, Sos1Constraint


@dataclass
class Plant:
    position: Tuple[float, float]
    max_capacity: float


@dataclass
class Client:
    position: Tuple[float, float]
    demand: float


@dataclass
class Input:
    plants: List[Plant]
    clients: List[Client]

    @classmethod
    def random(cls, num_plants: int, num_clients: int) -> "Input":
        """Sample a random instance that is feasible under the one-plant-per-region rule.

        Plant and client positions are drawn uniformly from :math:`[0, 100]^2`.
        Client demands are uniform on :math:`[200, 400]`. Plant capacities are
        drawn from a range sized relative to total demand.

        To keep the sampled instance feasible under "at most one plant per
        region", two repairs are applied:

        1. Plant positions are resampled until both west and east regions
           contain at least one plant.
        2. If the *best* plant in each region together cannot cover total
           demand, the deficit is split evenly between those two best plants.
           Only the two best plants are touched — all other capacities are
           left at their sampled values, so the benchmark difficulty is not
           inflated across the board.

        Requires ``num_plants >= 2``. Callers are expected to seed
        :mod:`random` before calling this.
        """
        if num_plants < 2:
            raise ValueError(
                "num_plants must be at least 2 to place one plant per region"
            )
        clients = [
            Client(
                position=(random.uniform(0, 100), random.uniform(0, 100)),
                demand=random.uniform(200, 400),
            )
            for _ in range(num_clients)
        ]
        total_demand = sum(c.demand for c in clients)
        lb = ceil(total_demand / 2)

        while True:
            plants = [
                Plant(
                    position=(random.uniform(0, 100), random.uniform(0, 100)),
                    max_capacity=random.uniform(2 * lb // 3, lb * 2),
                )
                for _ in range(num_plants)
            ]
            wests = [i for i, p in enumerate(plants) if p.position[0] < 50]
            easts = [i for i, p in enumerate(plants) if p.position[0] >= 50]
            if wests and easts:
                break

        max_w = max(wests, key=lambda i: plants[i].max_capacity)
        max_e = max(easts, key=lambda i: plants[i].max_capacity)
        deficit = total_demand - (
            plants[max_w].max_capacity + plants[max_e].max_capacity
        )
        if deficit > 0:
            bump = deficit / 2
            plants[max_w] = Plant(
                position=plants[max_w].position,
                max_capacity=plants[max_w].max_capacity + bump,
            )
            plants[max_e] = Plant(
                position=plants[max_e].position,
                max_capacity=plants[max_e].max_capacity + bump,
            )
        return cls(plants=plants, clients=clients)


def _dist(p: Plant, c: Client) -> float:
    return sqrt(
        (p.position[0] - c.position[0]) ** 2 + (p.position[1] - c.position[1]) ** 2
    )


def _west_indices(input: Input) -> List[int]:
    return [i for i, p in enumerate(input.plants) if p.position[0] < 50]


def _east_indices(input: Input) -> List[int]:
    return [i for i, p in enumerate(input.plants) if p.position[0] >= 50]


def _supply_capacity_vars(
    input: Input,
) -> Tuple[Dict[Tuple[int, int], DecisionVariable], Dict[int, DecisionVariable], int]:
    """Create the shared `s[i,j]` and `c[i]` variables with deterministic IDs.

    Returns ``(s, c, next_id)`` where ``next_id`` is the first unused variable ID
    for optional extra variables (e.g. big-M indicator binaries).
    """
    N = len(input.plants)
    M = len(input.clients)
    s: Dict[Tuple[int, int], DecisionVariable] = {}
    next_id = 0
    for i in range(N):
        for j in range(M):
            s[(i, j)] = DecisionVariable.continuous(
                next_id,
                lower=0.0,
                upper=input.clients[j].demand,
                name="s",
                subscripts=[i, j],
            )
            next_id += 1
    c: Dict[int, DecisionVariable] = {}
    for i in range(N):
        c[i] = DecisionVariable.continuous(
            next_id,
            lower=0.0,
            upper=input.plants[i].max_capacity,
            name="c",
            subscripts=[i],
        )
        next_id += 1
    return s, c, next_id


def _common_constraints(
    input: Input,
    s: Dict[Tuple[int, int], DecisionVariable],
    c: Dict[int, DecisionVariable],
) -> dict:
    """Capacity balance (per plant) and demand satisfaction (per client)."""
    N = len(input.plants)
    M = len(input.clients)
    constraints: dict = {}
    cid = 0
    for i in range(N):
        constraints[cid] = sum(s[(i, j)] for j in range(M)) - c[i] == 0  # type: ignore[assignment]
        cid += 1
    for j in range(M):
        constraints[cid] = sum(s[(i, j)] for i in range(N)) == input.clients[j].demand  # type: ignore[assignment]
        cid += 1
    return constraints


def _objective(
    input: Input,
    s: Dict[Tuple[int, int], DecisionVariable],
    c: Dict[int, DecisionVariable],
):
    N = len(input.plants)
    M = len(input.clients)
    transport = sum(
        _dist(input.plants[i], input.clients[j]) * s[(i, j)]
        for i in range(N)
        for j in range(M)
    )
    capacity = sum(c[i] for i in range(N))
    return transport + capacity


def build_sos1(input: Input) -> Instance:
    """Build the instance with one SOS1 constraint per region on :math:`c_i`.

    A region with fewer than two plants is trivially satisfied and is skipped.
    """
    s, c, _ = _supply_capacity_vars(input)
    constraints = _common_constraints(input, s, c)

    sos1_constraints: dict = {}
    sid = 0
    for group in (_west_indices(input), _east_indices(input)):
        if len(group) >= 2:
            sos1_constraints[sid] = Sos1Constraint(variables=[c[i].id for i in group])
            sid += 1

    return Instance.from_components(
        decision_variables=list(s.values()) + list(c.values()),
        objective=_objective(input, s, c),
        constraints=constraints,
        sos1_constraints=sos1_constraints,
        sense=Instance.MINIMIZE,
    )


def _build_with_delta(
    input: Input,
    *,
    with_cardinality: bool,
    with_sos1_on_c: bool,
    with_sos1_on_delta: bool,
) -> Instance:
    """Shared backbone for the seven builders that introduce ``delta`` binaries.

    All seven keep the big-M link ``c[i] <= C[i] * delta[i]``. They differ in
    whether the per-region cardinality ``sum delta[i] <= 1`` is added as a
    plain linear constraint and where SOS1 is declared (on ``c``, on
    ``delta``, on both, or nowhere).
    """
    s, c, next_id = _supply_capacity_vars(input)
    N = len(input.plants)

    delta = {
        i: DecisionVariable.binary(next_id + i, name="delta", subscripts=[i])
        for i in range(N)
    }

    constraints = _common_constraints(input, s, c)
    cid = max(constraints) + 1 if constraints else 0

    for i in range(N):
        constraints[cid] = c[i] - input.plants[i].max_capacity * delta[i] <= 0  # type: ignore[assignment]
        cid += 1

    if with_cardinality:
        for group in (_west_indices(input), _east_indices(input)):
            if group:
                constraints[cid] = sum(delta[i] for i in group) <= 1  # type: ignore[assignment]
                cid += 1

    sos1_constraints: dict = {}
    sid = 0
    if with_sos1_on_c:
        for group in (_west_indices(input), _east_indices(input)):
            if len(group) >= 2:
                sos1_constraints[sid] = Sos1Constraint(
                    variables=[c[i].id for i in group]
                )
                sid += 1
    if with_sos1_on_delta:
        for group in (_west_indices(input), _east_indices(input)):
            if len(group) >= 2:
                sos1_constraints[sid] = Sos1Constraint(
                    variables=[delta[i].id for i in group]
                )
                sid += 1

    return Instance.from_components(
        decision_variables=list(s.values()) + list(c.values()) + list(delta.values()),
        objective=_objective(input, s, c),
        constraints=constraints,
        sos1_constraints=sos1_constraints,
        sense=Instance.MINIMIZE,
    )


def build_bigm(input: Input) -> Instance:
    """Pure linear: big-M link plus per-region cardinality bounds, no SOS1."""
    return _build_with_delta(
        input,
        with_cardinality=True,
        with_sos1_on_c=False,
        with_sos1_on_delta=False,
    )


def build_sos1_on_delta(input: Input) -> Instance:
    """δ + big-M; per-region cardinality replaced by SOS1 on the binaries."""
    return _build_with_delta(
        input,
        with_cardinality=False,
        with_sos1_on_c=False,
        with_sos1_on_delta=True,
    )


def build_sos1_on_delta_with_card(input: Input) -> Instance:
    """δ + big-M + cardinality bounds AND a redundant SOS1 on the binaries."""
    return _build_with_delta(
        input,
        with_cardinality=True,
        with_sos1_on_c=False,
        with_sos1_on_delta=True,
    )


def build_sos1_on_c_with_delta(input: Input) -> Instance:
    """δ + big-M; per-region cardinality enforced by SOS1 on the continuous c_i."""
    return _build_with_delta(
        input,
        with_cardinality=False,
        with_sos1_on_c=True,
        with_sos1_on_delta=False,
    )


def build_sos1_on_c_with_delta_with_card(input: Input) -> Instance:
    """δ + big-M + cardinality AND a SOS1 on the continuous c_i (cardinality kept)."""
    return _build_with_delta(
        input,
        with_cardinality=True,
        with_sos1_on_c=True,
        with_sos1_on_delta=False,
    )


def build_sos1_on_both_with_delta(input: Input) -> Instance:
    """δ + big-M; SOS1 declared on both c and δ (no explicit cardinality)."""
    return _build_with_delta(
        input,
        with_cardinality=False,
        with_sos1_on_c=True,
        with_sos1_on_delta=True,
    )


def build_sos1_on_both_with_delta_with_card(input: Input) -> Instance:
    """δ + big-M + cardinality; SOS1 declared on both c and δ (maximum-information)."""
    return _build_with_delta(
        input,
        with_cardinality=True,
        with_sos1_on_c=True,
        with_sos1_on_delta=True,
    )
