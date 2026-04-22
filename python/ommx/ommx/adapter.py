from abc import ABC, abstractmethod
from typing import Any
from ommx.v1 import Instance, Solution, SampleSet, AdditionalCapability


SolverInput = Any
SolverOutput = Any
SamplerInput = Any
SamplerOutput = Any


class SolverAdapter(ABC):
    """
    An abstract interface for OMMX Solver Adapters, defining how solvers should be used with OMMX.

    See the `implementation guide <https://jij-inc-ommx.readthedocs-hosted.com/en/latest/tutorial/implement_adapter.html>`_ for more details.

    Subclasses should set ``ADDITIONAL_CAPABILITIES`` to declare which non-standard
    constraint types they can handle. Standard constraints are always supported.

    Available capabilities:

    - ``AdditionalCapability.Indicator``: binvar = 1 → f(x) <= 0
    - ``AdditionalCapability.OneHot``: exactly one of a set of binary variables is 1
    - ``AdditionalCapability.Sos1``: at most one of a set of variables is non-zero

    The default is an empty set (standard constraints only).
    Subclasses must call ``super().__init__(ommx_instance)`` so that any
    constraint types the adapter does not support are automatically converted
    into regular constraints (Big-M for indicator / SOS1, linear equality for
    one-hot). Conversions mutate ``ommx_instance`` in place and are emitted
    at ``INFO`` level as ``tracing`` events from the Rust SDK; configure a
    Python OpenTelemetry ``TracerProvider`` before the first call to observe
    them via ``pyo3-tracing-opentelemetry``.
    """

    ADDITIONAL_CAPABILITIES: frozenset[AdditionalCapability] = frozenset()

    def __init__(self, ommx_instance: Instance):
        """Reduce the instance to the adapter's supported capabilities.

        Subclasses must call ``super().__init__()``. Any constraint type not in
        ``ADDITIONAL_CAPABILITIES`` is converted to regular constraints in place
        on ``ommx_instance``.
        """
        ommx_instance.reduce_capabilities(set(self.ADDITIONAL_CAPABILITIES))

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
