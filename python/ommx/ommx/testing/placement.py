r"""Plant Placement Problem — two equivalent OMMX formulations.

This module provides a small, solver-agnostic benchmark problem used to
exercise an adapter's SOS1 handling. Both builders produce `ommx.v1.Instance`
objects describing the same feasible region and optimum; they differ only in
how "at most one plant per region" is communicated to the solver.

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

    s_{i,j} \in [0, d_j]    \qquad i \in 1..N,\ j \in 1..M \\
    c_i     \in [0, C_i]    \qquad i \in 1..N

where :math:`s_{i,j}` is the amount delivered from plant :math:`i` to client
:math:`j` and :math:`c_i` is the total capacity drawn from plant :math:`i`.

Shared constraints
~~~~~~~~~~~~~~~~~~

.. math::

    \sum_{j=1}^M s_{i,j} = c_i \quad &\text{(capacity balance, per plant)} \\
    \sum_{i=1}^N s_{i,j} = d_j \quad &\text{(demand, per client)}

Objective (minimize)
~~~~~~~~~~~~~~~~~~~~

.. math::

    \min \; \sum_{i,j} \operatorname{dist}(i, j) \cdot s_{i,j}
          \;+\; \sum_i c_i

"At most one plant per region" — two formulations
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

**SOS1 (first-class)** — :func:`build_sos1`.

The set :math:`\{c_i\}_{i \in W}` is marked as a single SOS1 constraint; the
same for :math:`\{c_i\}_{i \in E}`. Because :math:`c_i \ge 0`, SOS1 directly
encodes "at most one capacity is positive" without auxiliary binaries.
Adapters forward these to the solver (e.g. SCIP's ``addConsSOS1``).

**big-M (linearised)** — :func:`build_bigm`.

For each plant we introduce a binary :math:`\delta_i \in \{0, 1\}` acting as
an opening indicator, and add

.. math::

    c_i &\le C_i \, \delta_i \qquad \forall i \\
    \sum_{i \in W} \delta_i &\le 1 \\
    \sum_{i \in E} \delta_i &\le 1

No SOS1 constraint is produced; the solver sees only plain linear
constraints. The two formulations share the same projection onto
:math:`(s, c)` and therefore the same optimum.

Intended use
------------

These two builders are useful for measuring whether an adapter benefits from
forwarding SOS1 natively versus letting the user pre-linearise it. Callers
should construct :meth:`Input.random` with a fixed ``random.seed`` for
reproducibility and pass the resulting :class:`Input` to both builders to
obtain comparable instances.
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
        """Sample a random instance.

        Plant and client positions are drawn uniformly from :math:`[0, 100]^2`.
        Client demands are uniform on :math:`[200, 400]`. Plant capacities are
        sized relative to the total demand and then rescaled, if necessary, so
        that the "smallest-plant-in-each-region" lower bound still covers total
        demand — keeping the random instance feasible for the
        one-plant-per-region restriction.

        Callers are expected to seed :mod:`random` before calling this.
        """
        clients = [
            Client(
                position=(random.uniform(0, 100), random.uniform(0, 100)),
                demand=random.uniform(200, 400),
            )
            for _ in range(num_clients)
        ]
        total_demand = sum(c.demand for c in clients)
        lb = ceil(total_demand / 2)
        plants = [
            Plant(
                position=(random.uniform(0, 100), random.uniform(0, 100)),
                max_capacity=random.uniform(2 * lb // 3, lb * 2),
            )
            for _ in range(num_plants)
        ]
        wests = [p for p in plants if p.position[0] < 50]
        easts = [p for p in plants if p.position[0] >= 50]
        if wests and easts:
            min_capas = min(p.max_capacity for p in wests) + min(
                p.max_capacity for p in easts
            )
            if total_demand > min_capas:
                shift = total_demand - min_capas
                plants = [
                    Plant(position=p.position, max_capacity=p.max_capacity + shift)
                    for p in plants
                ]
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


def build_bigm(input: Input) -> Instance:
    """Build the big-M formulation with per-plant indicator binaries.

    This is the "native linear" equivalent: the solver sees no SOS1
    constraints — only plain linear inequalities tying each :math:`c_i` to its
    opening indicator :math:`\\delta_i`, and per-region cardinality bounds on
    the indicators.
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

    for group in (_west_indices(input), _east_indices(input)):
        if group:
            constraints[cid] = sum(delta[i] for i in group) <= 1  # type: ignore[assignment]
            cid += 1

    return Instance.from_components(
        decision_variables=list(s.values()) + list(c.values()) + list(delta.values()),
        objective=_objective(input, s, c),
        constraints=constraints,
        sense=Instance.MINIMIZE,
    )
