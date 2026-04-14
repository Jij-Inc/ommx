from abc import ABC, abstractmethod
from typing import Any
from ommx.v1 import Instance, Solution, SampleSet, ConstraintCapability


SolverInput = Any
SolverOutput = Any
SamplerInput = Any
SamplerOutput = Any


class SolverAdapter(ABC):
    """
    An abstract interface for OMMX Solver Adapters, defining how solvers should be used with OMMX.

    See the `implementation guide <https://jij-inc-ommx.readthedocs-hosted.com/en/latest/tutorial/implement_adapter.html>`_ for more details.

    Subclasses should set ``supported_constraints`` to declare which constraint
    types they can handle. Available capabilities:

    - ``ConstraintCapability.Standard``: f(x) = 0 or f(x) <= 0
    - ``ConstraintCapability.Indicator``: binvar = 1 → f(x) <= 0

    The default is ``{ConstraintCapability.Standard}`` only.
    Subclasses must call ``super().__init__(ommx_instance)`` to enable
    automatic constraint capability checking.
    """

    supported_constraints: set[ConstraintCapability] = {ConstraintCapability.Standard}

    def __init__(self, ommx_instance: Instance):
        """Check constraint capabilities. Subclasses must call super().__init__()."""
        ommx_instance.check_capabilities(self.supported_constraints)

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

    See the `implementation guide <https://jij-inc-ommx.readthedocs-hosted.com/en/latest/tutorial/implement_adapter.html>`_ for more details.
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


class InfeasibleDetected(Exception):
    """
    Raised when the problem is proven to be infeasible.

    This corresponds to ``Optimality.OPTIMALITY_INFEASIBLE`` and indicates that
    the mathematical model itself has no feasible solution.
    Should not be used when infeasibility cannot be proven (e.g., heuristic solvers).
    """

    pass


class UnboundedDetected(Exception):
    """
    Raised when the problem is proven to be unbounded.

    This corresponds to ``Optimality.OPTIMALITY_UNBOUNDED`` and indicates that
    the mathematical model itself is unbounded.
    Should not be used when unboundedness cannot be proven (e.g., heuristic solvers).
    """

    pass


class NoSolutionReturned(Exception):
    """
    Raised when no solution was returned.

    This indicates that the solver did not return any solution (whether feasible
    or not) (e.g., due to time limits).
    This does not prove that the mathematical model itself is infeasible.
    """

    pass
