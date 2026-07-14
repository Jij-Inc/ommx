from __future__ import annotations

import copy
from abc import ABC, abstractmethod
from collections.abc import Iterable
from dataclasses import dataclass, field
from typing import Any, ClassVar, Protocol, runtime_checkable

from ommx._ommx_rust import DiagnosticCollector as DiagnosticCollector
from ommx import (
    AdapterCapabilities,
    Instance,
    PortableCompatibilityReport,
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
class ConstraintRef:
    """Constraint identity qualified by its independently scoped family."""

    family: str
    id: int


PreconditionValue = str | int | float | bool | None


@dataclass(frozen=True, slots=True)
class AdapterPreconditionViolation:
    """One adapter-owned condition that the portable profile cannot express."""

    condition: str
    description: str
    variable_ids: frozenset[int] = field(default_factory=frozenset)
    constraint_refs: frozenset[ConstraintRef] = field(default_factory=frozenset)
    actual: PreconditionValue = None
    limit: PreconditionValue = None


@dataclass(frozen=True, slots=True)
class AdapterCompatibilityReport:
    """Combined portable and adapter-specific compatibility result."""

    adapter: str
    portable_report: PortableCompatibilityReport
    preconditions_checked: bool
    precondition_violations: tuple[AdapterPreconditionViolation, ...]

    @property
    def compatible(self) -> bool:
        return (
            self.portable_report.compatible
            and self.preconditions_checked
            and not self.precondition_violations
        )

    def __str__(self) -> str:
        if not self.portable_report.compatible:
            return f"{self.adapter} is incompatible:\n{self.portable_report}"
        if self.precondition_violations:
            details = "\n".join(
                f"- {violation.condition}: {violation.description}"
                for violation in self.precondition_violations
            )
            return f"{self.adapter} preconditions failed:\n{details}"
        return f"{self.adapter} is compatible"


class AdapterCompatibilityError(ValueError):
    """Raised when an instance is incompatible with an adapter."""

    report: AdapterCompatibilityReport

    def __init__(self, report: AdapterCompatibilityReport):
        self.report = report
        super().__init__(str(report))


class SolverAdapter(ABC):
    """
    An abstract interface for OMMX Solver Adapters, defining how solvers should be used with OMMX.

    See the `implementation guide <https://jij-inc-ommx.readthedocs-hosted.com/en/latest/tutorial/implement_adapter.html>`_ for more details.

    Subclasses using the portable compatibility API declare ``CAPABILITIES`` as
    one or more complete native translator profiles. ``check_compatibility``
    does not mutate the input instance and combines that portable comparison
    with the adapter's ``_check_preconditions`` hook.

    Any transformation needed before translation is an explicit,
    adapter-owned preparation operation. The base adapter never mutates the
    input instance.
    """

    CAPABILITIES: ClassVar[AdapterCapabilities | None] = None

    @classmethod
    def check_compatibility(cls, ommx_instance: Instance) -> AdapterCompatibilityReport:
        """Inspect compatibility without mutating or preparing ``ommx_instance``.

        Adapter-specific preconditions run only after at least one complete
        portable profile matches. The hook receives an isolated copy so it
        cannot mutate the caller's instance. Preparation belongs to a separate
        explicit operation followed by another check.
        """
        capabilities = cls.CAPABILITIES
        if capabilities is None:
            raise TypeError(
                f"{cls.__module__}.{cls.__qualname__} must declare CAPABILITIES"
            )

        portable_report = capabilities.check_compatibility(
            ommx_instance.solver_requirements()
        )
        adapter = f"{cls.__module__}.{cls.__qualname__}"
        if not portable_report.compatible:
            return AdapterCompatibilityReport(
                adapter=adapter,
                portable_report=portable_report,
                preconditions_checked=False,
                precondition_violations=(),
            )

        violations = tuple(
            cls._check_preconditions(copy.copy(ommx_instance), portable_report)
        )
        if not all(
            isinstance(violation, AdapterPreconditionViolation)
            for violation in violations
        ):
            raise TypeError(
                f"{adapter}._check_preconditions() must return "
                "AdapterPreconditionViolation values"
            )
        return AdapterCompatibilityReport(
            adapter=adapter,
            portable_report=portable_report,
            preconditions_checked=True,
            precondition_violations=violations,
        )

    @classmethod
    def require_compatible(cls, ommx_instance: Instance) -> AdapterCompatibilityReport:
        """Return the compatibility report or raise ``AdapterCompatibilityError``."""
        report = cls.check_compatibility(ommx_instance)
        if not report.compatible:
            raise AdapterCompatibilityError(report)
        return report

    @classmethod
    def _check_preconditions(
        cls,
        ommx_instance: Instance,
        portable_report: PortableCompatibilityReport,
    ) -> Iterable[AdapterPreconditionViolation]:
        """Return adapter-owned violations after a portable profile matches."""
        return ()

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
