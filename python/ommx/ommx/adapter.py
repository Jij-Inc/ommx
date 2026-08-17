from __future__ import annotations

from abc import ABC, abstractmethod
from dataclasses import dataclass
from typing import Any, ClassVar, Protocol, runtime_checkable

from ommx._ommx_rust import DiagnosticCollector as DiagnosticCollector
from ommx import (
    InfeasibleDetected as InfeasibleDetected,
    Instance,
    InstanceClass,
    InstanceClassMembershipReport,
    PreparationPolicy,
    SampleSet,
    Solution,
)


SolverInput = Any
SolverOutput = Any
SamplerInput = Any
SamplerOutput = Any


@runtime_checkable
class DiagnosticReport(Protocol):
    """Adapter diagnostic report convertible with ``dataclasses.asdict``."""

    __dataclass_fields__: ClassVar[dict[str, Any]]


class DiagnosticsSink(Protocol):
    """Receiver for adapter-defined diagnostics emitted during a solve.

    Adapters may call ``record`` while the backend solver is still running,
    including from backend callbacks. Sink implementations should keep
    ``record`` append-only, defer validation or serialization until after the
    solve, and preserve the order in which diagnostics are received.

    A conforming sink must not raise from ``record``. If recording fails, the
    sink should log the failure and return normally. If ``record`` does raise,
    that is a sink contract violation; adapters may let the exception propagate
    and do not need to recover from it.
    """

    def record(self, diagnostic: DiagnosticReport) -> None:
        """Record one adapter-defined dataclass diagnostic report or event.

        This method must not raise under normal sink failures. Custom sinks
        should log failures and return instead.
        """


@dataclass(frozen=True, slots=True)
class AdapterApplicabilityReport:
    """Applicability result defined by an Adapter's input class."""

    adapter: str
    input_membership: InstanceClassMembershipReport

    @property
    def is_applicable(self) -> bool:
        return self.input_membership.is_member

    def __str__(self) -> str:
        if not self.input_membership.is_member:
            return f"{self.adapter} is not applicable:\n{self.input_membership}"
        return f"{self.adapter} is applicable"


class AdapterNotApplicableError(ValueError):
    """Raised when an instance is not applicable to an adapter."""

    report: AdapterApplicabilityReport

    def __init__(self, report: AdapterApplicabilityReport):
        self.report = report
        super().__init__(str(report))


class SolverAdapter(ABC):
    """
    An abstract interface for OMMX Solver Adapters, defining how solvers should be used with OMMX.

    See the `implementation guide <https://jij-inc-ommx.readthedocs-hosted.com/en/latest/tutorial/implement_adapter.html>`_ for more details.

    Concrete subclasses define applicability with ``INPUT_CLASS``; callers own
    applying any recommended policy with :meth:`ommx.Instance.prepare`.
    """

    INPUT_CLASS: ClassVar[InstanceClass]
    """Required condition for an exact Adapter input."""

    @classmethod
    def recommended_preparation_policy(cls) -> PreparationPolicy:
        """Return a fresh policy recommended for this Adapter's ``INPUT_CLASS``.

        The caller owns editing and applying it. This method neither prepares an
        instance nor guarantees applicability. The default policy is empty.
        """
        return PreparationPolicy()

    @classmethod
    def check_applicability(cls, ommx_instance: Instance) -> AdapterApplicabilityReport:
        """Check ``INPUT_CLASS`` membership without mutation."""
        input_class: InstanceClass | None = getattr(cls, "INPUT_CLASS", None)
        if input_class is None:
            raise TypeError(
                f"{cls.__module__}.{cls.__qualname__} must declare INPUT_CLASS"
            )

        input_membership = input_class.check_membership(ommx_instance)
        adapter = f"{cls.__module__}.{cls.__qualname__}"
        return AdapterApplicabilityReport(
            adapter=adapter,
            input_membership=input_membership,
        )

    @classmethod
    def require_applicable(cls, ommx_instance: Instance) -> AdapterApplicabilityReport:
        """Return the report or raise :class:`AdapterNotApplicableError`."""
        report = cls.check_applicability(ommx_instance)
        if not report.is_applicable:
            raise AdapterNotApplicableError(report)
        return report

    @classmethod
    @abstractmethod
    def solve(
        cls,
        ommx_instance: Instance,
        *,
        diagnostics: DiagnosticsSink | None = None,
    ) -> Solution:
        """Solve an OMMX instance.

        ``Run.log_solve`` owns the reserved ``diagnostics`` keyword. When
        called with ``store_diagnostics=True``, it passes a sink to the adapter
        and stores recorded diagnostics with the Solve entry. Adapters may
        record adapter-defined dataclass diagnostics into the sink during the
        solve; ``None`` means diagnostics are disabled. Adapters do not need to
        catch exceptions raised by a non-conforming diagnostics sink.
        """
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
    def sample(
        cls,
        ommx_instance: Instance,
        *,
        diagnostics: DiagnosticsSink | None = None,
    ) -> SampleSet:
        """Sample an OMMX instance.

        ``Run.log_sample`` owns the reserved ``diagnostics`` keyword and uses
        it the same way as ``Run.log_solve``. ``None`` means diagnostics are
        disabled.
        """
        pass

    @property
    @abstractmethod
    def sampler_input(self) -> SamplerInput:
        pass

    @abstractmethod
    def decode_to_sampleset(self, data: SamplerOutput) -> SampleSet:
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
