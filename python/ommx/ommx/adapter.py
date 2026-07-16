from __future__ import annotations

import copy
from abc import ABC, abstractmethod
from collections.abc import Iterable
from dataclasses import dataclass, field
from typing import Any, ClassVar, Protocol, runtime_checkable

from ommx._ommx_rust import DiagnosticCollector as DiagnosticCollector
from ommx import (
    AdditionalCapability,
    Instance,
    InstanceClass,
    InstanceClassMembershipReport,
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
    """One adapter-owned condition that an OMMX input class cannot express."""

    condition: str
    description: str
    variable_ids: frozenset[int] = field(default_factory=frozenset)
    constraint_refs: frozenset[ConstraintRef] = field(default_factory=frozenset)
    actual: PreconditionValue = None
    limit: PreconditionValue = None


@dataclass(frozen=True, slots=True)
class AdapterApplicabilityReport:
    """Combined input-class and adapter-specific applicability result."""

    adapter: str
    input_membership: InstanceClassMembershipReport
    preconditions_checked: bool
    precondition_violations: tuple[AdapterPreconditionViolation, ...]

    def __post_init__(self) -> None:
        if self.preconditions_checked != self.input_membership.is_member:
            raise ValueError(
                "preconditions_checked must be true exactly when input membership holds"
            )
        if not self.preconditions_checked and self.precondition_violations:
            raise ValueError(
                "precondition violations require adapter preconditions to be checked"
            )

    @property
    def is_applicable(self) -> bool:
        return (
            self.input_membership.is_member
            and self.preconditions_checked
            and not self.precondition_violations
        )

    def __str__(self) -> str:
        if not self.input_membership.is_member:
            return f"{self.adapter} is not applicable:\n{self.input_membership}"
        if self.precondition_violations:
            details = "\n".join(
                f"- {violation.condition}: {violation.description}"
                for violation in self.precondition_violations
            )
            return f"{self.adapter} preconditions failed:\n{details}"
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

    Subclasses declare ``INPUT_CLASS`` as the OMMX-defined structural class used
    by the first applicability condition. ``check_applicability`` does not mutate
    the input and combines class membership with the adapter's
    ``_check_preconditions`` hook.

    ``ADDITIONAL_CAPABILITIES`` and the base constructor remain temporarily for
    adapters that still use the legacy in-place special-constraint lowering path.
    It is independent of ``INPUT_CLASS`` and is not used as a fallback.

    Legacy special-constraint flags:

    - ``AdditionalCapability.Indicator``: binvar = 1 → f(x) <= 0
    - ``AdditionalCapability.OneHot``: exactly one of a set of binary variables is 1
    - ``AdditionalCapability.Sos1``: at most one of a set of variables is non-zero

    Legacy subclasses must call ``super().__init__(ommx_instance)`` so that any
    special-constraint family not preserved by the lowering selector is
    converted into regular constraints (Big-M for indicator / SOS1, linear
    equality for one-hot). This operation does not establish ``INPUT_CLASS``
    membership or adapter applicability. Conversions mutate ``ommx_instance``
    in place and are emitted at ``INFO`` level as ``tracing`` events from the
    Rust SDK; configure a Python OpenTelemetry ``TracerProvider`` before the
    first call to observe them via ``pyo3-tracing-opentelemetry``.
    """

    INPUT_CLASS: ClassVar[InstanceClass | None] = None
    ADDITIONAL_CAPABILITIES: ClassVar[frozenset[AdditionalCapability]] = frozenset()

    def __init__(self, ommx_instance: Instance):
        """Apply the legacy special-constraint lowering selector.

        Subclasses using this legacy path must call ``super().__init__()``. Any
        active special-constraint family not in ``ADDITIONAL_CAPABILITIES`` is
        converted to regular constraints in place on ``ommx_instance``. The
        resulting input must still be checked against ``INPUT_CLASS``.
        """
        ommx_instance.reduce_capabilities(set(self.ADDITIONAL_CAPABILITIES))

    @classmethod
    def check_applicability(cls, ommx_instance: Instance) -> AdapterApplicabilityReport:
        """Inspect applicability without mutating or preparing ``ommx_instance``.

        Adapter-specific preconditions run only after at least one complete
        input-class clause contains the instance. The hook receives an isolated
        copy so it cannot mutate the caller's instance. Preparation belongs to
        a separate explicit operation followed by another membership check.
        """
        input_class = cls.INPUT_CLASS
        if input_class is None:
            raise TypeError(
                f"{cls.__module__}.{cls.__qualname__} must declare INPUT_CLASS"
            )

        input_membership = input_class.check_membership(ommx_instance)
        adapter = f"{cls.__module__}.{cls.__qualname__}"
        if not input_membership.is_member:
            return AdapterApplicabilityReport(
                adapter=adapter,
                input_membership=input_membership,
                preconditions_checked=False,
                precondition_violations=(),
            )

        violations = tuple(
            cls._check_preconditions(copy.copy(ommx_instance), input_membership)
        )
        if not all(
            isinstance(violation, AdapterPreconditionViolation)
            for violation in violations
        ):
            raise TypeError(
                f"{adapter}._check_preconditions() must return "
                "AdapterPreconditionViolation values"
            )
        return AdapterApplicabilityReport(
            adapter=adapter,
            input_membership=input_membership,
            preconditions_checked=True,
            precondition_violations=violations,
        )

    @classmethod
    def require_applicable(cls, ommx_instance: Instance) -> AdapterApplicabilityReport:
        """Return the report or raise :class:`AdapterNotApplicableError`."""
        report = cls.check_applicability(ommx_instance)
        if not report.is_applicable:
            raise AdapterNotApplicableError(report)
        return report

    @classmethod
    def _check_preconditions(
        cls,
        ommx_instance: Instance,
        input_membership: InstanceClassMembershipReport,
    ) -> Iterable[AdapterPreconditionViolation]:
        """Return adapter-owned violations after input-class membership holds."""
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
