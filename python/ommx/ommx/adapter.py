from abc import ABC, abstractmethod
from typing import Any
from ommx.v1 import Instance, Solution, SampleSet


SolverInput = Any
SolverOutput = Any
SamplerInput = Any
SamplerOutput = Any


class SolverAdapter(ABC):
    """
    An abstract interface for OMMX Solver Adapters, defining how solvers should be used with OMMX.

    See the `implementation guide <https://jij-inc.github.io/ommx/en/user_guide/solver_adapter.html>`_ for more details.
    """

    @abstractmethod
    def __init__(self, ommx_instance: Instance):
        pass

    @classmethod
    @abstractmethod
    def solve(cls, ommx_instance: Instance) -> Solution:
        pass

    @property
    @abstractmethod
    def solver_input(self) -> SolverInput:
        pass

    @abstractmethod
    def decode(self, data: SolverOutput) -> Solution:
        pass


class SamplerAdapter(SolverAdapter):
    """
    An abstract interface for OMMX Sampler Adapters, defining how samplers should be used with OMMX.

    See the `implementation guide <https://jij-inc.github.io/ommx/en/user_guide/solver_adapter.html>`_ for more details.
    """

    @classmethod
    @abstractmethod
    def sample(cls, ommx_instance: Instance) -> SampleSet:
        pass

    @property
    @abstractmethod
    def sampler_input(self) -> SamplerInput:
        pass

    @abstractmethod
    def decode_to_sampleset(self, data: SamplerOutput) -> SampleSet:
        pass

    @classmethod
    def solve(cls, ommx_instance: Instance, **kwargs) -> Solution:
        solution = cls.sample(ommx_instance, **kwargs).best_feasible()
        if solution is None:
            raise NoFeasibleSample("No feasible sample found by the sampler.")
        return solution

    @property
    def solver_input(self) -> SamplerInput:
        return self.sampler_input

    def decode(self, data: SamplerOutput) -> Solution:
        solution = self.decode_to_sampleset(data).best_feasible()
        if solution is None:
            raise NoFeasibleSample("No feasible sample found by the sampler.")
        return solution


class InfeasibleDetected(Exception):
    pass


class UnboundedDetected(Exception):
    pass


class NoFeasibleSample(Exception):
    """
    Exception raised when no feasible solution is found by the solver.
    This is a subclass of `Exception` and can be caught in the usual way.
    """
    pass
