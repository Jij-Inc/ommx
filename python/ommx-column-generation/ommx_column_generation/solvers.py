from __future__ import annotations

from ommx.v1 import Instance, Solution
from ommx_highs_adapter import OMMXHighsAdapter


def highs_master_solver(instance: Instance) -> Solution:
    """Solve an RMP instance with HiGHS."""

    return OMMXHighsAdapter.solve(instance)
