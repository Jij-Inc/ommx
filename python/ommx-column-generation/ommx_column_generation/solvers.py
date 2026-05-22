from __future__ import annotations

from ommx.v1 import Instance, Solution
from ommx_highs_adapter import OMMXHighsAdapter


def highs_master_solver(instance: Instance) -> Solution:
    r"""Solve an RMP instance with HiGHS.

    This is a ``MasterSolver`` implementation for the LP RMP

    .. math::

       \min c_0 + \sum_j c_j \lambda_j.

    The returned ``Solution`` is expected to contain primal values
    :math:`\lambda_j` and dual values for the RMP rows.
    """

    return OMMXHighsAdapter.solve(instance)
