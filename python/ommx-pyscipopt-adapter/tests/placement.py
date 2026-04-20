"""Plant Placement Problem used by placement benchmarks and equivalence tests.

Builds the same underlying model in two forms:

- ``build_sos1``:  uses ``ommx.v1.Sos1Constraint`` so the adapter calls ``addConsSOS1``.
- ``build_bigm``:  expresses "at most one plant per region" with binary indicator
  variables and big-M linearisation, so no SOS1 reaches SCIP.

Both formulations share decision variables ``s[i,j]`` (supply from plant ``i``
to client ``j``) and ``c[i]`` (capacity used at plant ``i``). ``build_bigm``
additionally introduces ``delta[i]`` binaries.
"""

from __future__ import annotations

from dataclasses import dataclass
from math import ceil, sqrt
import random
from typing import List, Tuple

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
        # Ensure the problem is feasible: the smallest plant in each region
        # plus the smallest in the other region must cover total demand.
        wests = [p for p in plants if p.position[0] < 50]
        easts = [p for p in plants if p.position[0] >= 50]
        if wests and easts:
            min_capas = min(p.max_capacity for p in wests) + min(
                p.max_capacity for p in easts
            )
            if total_demand > min_capas:
                delta = total_demand - min_capas
                plants = [
                    Plant(position=p.position, max_capacity=p.max_capacity + delta)
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
) -> Tuple[dict, dict, int]:
    """Create ``s[i,j]`` and ``c[i]`` variables with deterministic IDs.

    Returns ``(s, c, next_id)`` where ``next_id`` is the first unused variable ID.
    """
    N = len(input.plants)
    M = len(input.clients)
    s: dict = {}
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
    c: dict = {}
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


def _common_constraints(input: Input, s: dict, c: dict) -> dict:
    """Supply==capacity per plant and supply==demand per client."""
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


def _objective(input: Input, s: dict, c: dict):
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
    """Model "at most one plant per region" via first-class SOS1 on the capacity vars."""
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
    """Model the same logic with delta binaries and big-M constraints (no SOS1)."""
    s, c, next_id = _supply_capacity_vars(input)
    N = len(input.plants)

    delta = {
        i: DecisionVariable.binary(
            next_id + i, name="delta", subscripts=[i]
        )
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
