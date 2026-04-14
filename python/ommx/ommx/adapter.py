from abc import ABC, abstractmethod
from enum import Flag, auto
from typing import Any
from ommx.v1 import Instance, Solution, SampleSet


SolverInput = Any
SolverOutput = Any
SamplerInput = Any
SamplerOutput = Any


class ConstraintCapability(Flag):
    """Flags indicating which constraint types an adapter supports."""

    #: Standard constraints: f(x) = 0 or f(x) <= 0
    STANDARD = auto()
    #: Indicator constraints: binvar = 1 → f(x) <= 0
    INDICATOR = auto()


class UnsupportedConstraintType(Exception):
    """Raised when an Instance contains constraint types not supported by the adapter."""

    pass


class SolverAdapter(ABC):
    """
    An abstract interface for OMMX Solver Adapters, defining how solvers should be used with OMMX.

    See the `implementation guide <https://jij-inc.github.io/ommx/en/user_guide/solver_adapter.html>`_ for more details.

    Subclasses should set ``supported_constraints`` to declare which constraint
    types they can handle. The default is ``ConstraintCapability.STANDARD`` only.
    Subclasses must call ``super().__init__(ommx_instance)`` to enable
    automatic constraint capability checking.
    """

    supported_constraints: ConstraintCapability = ConstraintCapability.STANDARD

    def __init__(self, ommx_instance: Instance):
        """Check constraint capabilities. Subclasses must call super().__init__()."""
        self._check_constraint_capabilities(ommx_instance)

    def _check_constraint_capabilities(self, instance: Instance) -> None:
        """Raise UnsupportedConstraintType if the instance uses unsupported constraint types."""
        if (
            len(instance.indicator_constraints) > 0
            and not (self.supported_constraints & ConstraintCapability.INDICATOR)
        ):
            raise UnsupportedConstraintType(
                f"{self.__class__.__name__} does not support indicator constraints. "
                f"Found {len(instance.indicator_constraints)} indicator constraint(s)."
            )

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
