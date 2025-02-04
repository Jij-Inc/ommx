from abc import ABC, abstractmethod
from typing import Any
from ommx.v1 import Instance, Solution


SolverInput = Any
SolverOutput = Any


class SolverAdapter(ABC):
    """
    An abstract interface for Adapters, defining how solvers should be used with OMMX.

    See the `implementation guide` for more details.
    .. _implementation guide: https://jij-inc.github.io/ommx/ommx_ecosystem/solver_adapter_guide.html
    """

    @abstractmethod
    def __init__(self, ommx_instance: Instance):
        pass

    @staticmethod
    @abstractmethod
    def solve(ommx_instance: Instance) -> Solution:
        pass

    @property
    @abstractmethod
    def solver_input(self) -> SolverInput:
        pass

    @abstractmethod
    def decode(self, data: SolverOutput) -> Solution:
        pass


class InfeasibleDetected(Exception):
    pass


class UnboundedDetected(Exception):
    pass
