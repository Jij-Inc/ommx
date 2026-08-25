from __future__ import annotations

import copy
from abc import ABC, abstractmethod
from collections.abc import Iterator
from contextlib import contextmanager
from contextvars import ContextVar
from typing import Any, ClassVar, Protocol, runtime_checkable

from opentelemetry.trace import Tracer

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


class AdapterNotApplicableError(ValueError):
    """Raised when an instance is not applicable to an adapter."""

    adapter: str
    report: InstanceClassMembershipReport

    def __init__(self, adapter: str, report: InstanceClassMembershipReport):
        self.adapter = adapter
        self.report = report
        super().__init__(f"{adapter} is not applicable:\n{report}")


class SolverAdapter(ABC):
    """
    An abstract interface for OMMX Solver Adapters, defining how solvers should be used with OMMX.

    See the `implementation guide <https://jij-inc-ommx.readthedocs-hosted.com/en/latest/tutorial/implement_adapter.html>`_ for more details.

    Concrete subclasses define applicability with ``INPUT_CLASS``. The easy
    :meth:`solve` API prepares an isolated copy with the Adapter's recommended
    policy. Use :meth:`solve_without_preparation` when the caller owns preparation and wants
    the Adapter to require an exact input without modifying it.
    """

    INPUT_CLASS: ClassVar[InstanceClass]
    """Required condition for an exact Adapter input."""

    @classmethod
    def recommended_preparation_policy(cls) -> PreparationPolicy:
        """Return a fresh policy recommended for this Adapter's ``INPUT_CLASS``.

        The easy APIs apply it to an isolated copy. Advanced callers may edit
        and apply it explicitly before using a preparation-free API. This
        method itself neither prepares an instance nor guarantees
        applicability. The default policy is empty.
        """
        return PreparationPolicy()

    @classmethod
    def check_applicability(
        cls, ommx_instance: Instance
    ) -> InstanceClassMembershipReport:
        """Check ``INPUT_CLASS`` membership without mutation."""
        input_class: InstanceClass | None = getattr(cls, "INPUT_CLASS", None)
        if input_class is None:
            raise TypeError(
                f"{cls.__module__}.{cls.__qualname__} must declare INPUT_CLASS"
            )

        return input_class.check_membership(ommx_instance)

    @classmethod
    def require_applicable(
        cls, ommx_instance: Instance
    ) -> InstanceClassMembershipReport:
        """Return the membership report or raise ``AdapterNotApplicableError``."""
        report = cls.check_applicability(ommx_instance)
        if not report.is_member:
            adapter = f"{cls.__module__}.{cls.__qualname__}"
            raise AdapterNotApplicableError(adapter, report)
        return report

    @classmethod
    def solve(
        cls,
        ommx_instance: Instance,
        *,
        diagnostics: DiagnosticsSink | None = None,
    ) -> Solution:
        """Prepare and solve an isolated copy of an OMMX instance.

        The input ``ommx_instance`` is never modified. The copy is prepared for
        ``INPUT_CLASS`` with :meth:`recommended_preparation_policy`, then passed
        to :meth:`solve_without_preparation`.

        ``Run.log_solve`` owns the reserved ``diagnostics`` keyword. When
        called with ``store_diagnostics=True``, it passes a sink to the adapter
        and stores recorded diagnostics with the Solve entry. Adapters may
        record adapter-defined dataclass diagnostics into the sink during the
        solve; ``None`` means diagnostics are disabled. Adapters do not need to
        catch exceptions raised by a non-conforming diagnostics sink.
        """
        prepared = copy.copy(ommx_instance)
        prepared.prepare(cls.INPUT_CLASS, cls.recommended_preparation_policy())
        return cls.solve_without_preparation(prepared, diagnostics=diagnostics)

    @classmethod
    @abstractmethod
    def solve_without_preparation(
        cls,
        ommx_instance: Instance,
        *,
        diagnostics: DiagnosticsSink | None = None,
    ) -> Solution:
        """Solve an exact Adapter input without running ``Instance.prepare``.

        ``ommx_instance`` must belong to ``INPUT_CLASS``. Implementations must
        reject non-members with :class:`AdapterNotApplicableError` and must not
        prepare or otherwise modify the input instance.
        """
        pass

    @property
    @abstractmethod
    def solver_input(self) -> SolverInput:
        pass

    @abstractmethod
    def decode(self, data: SolverOutput) -> Solution:
        pass


_active_adapter_execution_spans: ContextVar[
    frozenset[tuple[type[SolverAdapter], str]]
] = ContextVar("active_adapter_execution_spans", default=frozenset())


@contextmanager
def _adapter_execution_span(
    tracer: Tracer,
    operation: str,
    adapter_cls: type[SolverAdapter],
) -> Iterator[None]:
    """Create one span for a possibly nested Adapter API operation."""
    key = (adapter_cls, operation)
    active = _active_adapter_execution_spans.get()
    if key in active:
        yield
        return

    token = _active_adapter_execution_spans.set(active | {key})
    try:
        with tracer.start_as_current_span(operation) as span:
            span.set_attribute(
                "adapter", f"{adapter_cls.__module__}.{adapter_cls.__qualname__}"
            )
            yield
    finally:
        _active_adapter_execution_spans.reset(token)


class SamplerAdapter(SolverAdapter):
    """
    An abstract interface for OMMX Sampler Adapters, defining how samplers should be used with OMMX.

    See the `implementation guide <https://jij-inc-ommx.readthedocs-hosted.com/en/latest/tutorial/implement_adapter.html>`_ for more details.
    """

    @classmethod
    def sample(
        cls,
        ommx_instance: Instance,
        *,
        diagnostics: DiagnosticsSink | None = None,
    ) -> SampleSet:
        """Prepare and sample an isolated copy of an OMMX instance.

        The input ``ommx_instance`` is never modified. The copy is prepared for
        ``INPUT_CLASS`` with :meth:`recommended_preparation_policy`, then passed
        to :meth:`sample_without_preparation`.

        ``Run.log_sample`` owns the reserved ``diagnostics`` keyword and uses
        it the same way as ``Run.log_solve``. ``None`` means diagnostics are
        disabled.
        """
        prepared = copy.copy(ommx_instance)
        prepared.prepare(cls.INPUT_CLASS, cls.recommended_preparation_policy())
        return cls.sample_without_preparation(prepared, diagnostics=diagnostics)

    @classmethod
    @abstractmethod
    def sample_without_preparation(
        cls,
        ommx_instance: Instance,
        *,
        diagnostics: DiagnosticsSink | None = None,
    ) -> SampleSet:
        """Sample an exact Adapter input without running ``Instance.prepare``.

        ``ommx_instance`` must belong to ``INPUT_CLASS``. Implementations must
        reject non-members with :class:`AdapterNotApplicableError` and must not
        prepare or otherwise modify the input instance.
        """
        pass

    @classmethod
    def solve_without_preparation(
        cls,
        ommx_instance: Instance,
        *,
        diagnostics: DiagnosticsSink | None = None,
    ) -> Solution:
        """Return the best feasible result from :meth:`sample_without_preparation`."""
        return cls.sample_without_preparation(
            ommx_instance,
            diagnostics=diagnostics,
        ).best_feasible

    @property
    def solver_input(self) -> SolverInput:
        """Expose :attr:`sampler_input` through the SolverAdapter interface."""
        return self.sampler_input

    def decode(self, data: SolverOutput) -> Solution:
        """Decode sampler output and return its best feasible solution."""
        return self.decode_to_sampleset(data).best_feasible

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
