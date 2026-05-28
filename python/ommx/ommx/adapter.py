from __future__ import annotations

import json
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from typing import Any, ClassVar, Mapping, Protocol

from ommx.v1 import Instance, Solution, SampleSet, AdditionalCapability


SolverInput = Any
SolverOutput = Any
SamplerInput = Any
SamplerOutput = Any


@dataclass(slots=True)
class DiagnosticEntry:
    """One solver diagnostic payload emitted by an adapter.

    Diagnostics are adapter-owned evidence such as native solver reports,
    termination summaries, or timelines. OMMX core treats the payload as
    bytes plus media type and annotations; adapters define the solver-specific
    schema carried by ``data``.
    """

    name: str
    media_type: str
    data: bytes
    annotations: Mapping[str, str] = field(default_factory=dict)

    def __post_init__(self) -> None:
        if not self.name:
            msg = "DiagnosticEntry.name must not be empty"
            raise ValueError(msg)
        if not self.media_type:
            msg = "DiagnosticEntry.media_type must not be empty"
            raise ValueError(msg)
        base_media_type = self.media_type.split(";", 1)[0].strip()
        if "/" not in base_media_type or any(
            not part for part in base_media_type.split("/", 1)
        ):
            msg = "DiagnosticEntry.media_type must be a valid media type"
            raise ValueError(msg)
        if not isinstance(self.data, bytes):
            msg = "DiagnosticEntry.data must be bytes"
            raise TypeError(msg)

        annotations = dict(self.annotations)
        for key, value in annotations.items():
            if not isinstance(key, str) or not isinstance(value, str):
                msg = "DiagnosticEntry.annotations must map str keys to str values"
                raise TypeError(msg)
        self.annotations = annotations

    @classmethod
    def from_json(
        cls,
        name: str,
        value: Any,
        *,
        annotations: Mapping[str, str] | None = None,
        media_type: str = "application/json",
    ) -> DiagnosticEntry:
        """Create a diagnostic entry from a JSON-serializable value."""

        data = json.dumps(
            value,
            allow_nan=False,
            separators=(",", ":"),
            sort_keys=True,
        ).encode()
        return cls(name, media_type, data, annotations or {})


class DiagnosticsSink(Protocol):
    """Protocol consumed by adapters that can emit diagnostics."""

    def record(self, entry: DiagnosticEntry) -> None:
        """Record one diagnostic entry."""


class DiagnosticCollector:
    """In-memory diagnostics sink for direct adapter calls."""

    def __init__(self) -> None:
        self._entries: list[DiagnosticEntry] = []

    @property
    def entries(self) -> tuple[DiagnosticEntry, ...]:
        return tuple(self._entries)

    def record(self, entry: DiagnosticEntry) -> None:
        if not isinstance(entry, DiagnosticEntry):
            msg = "DiagnosticCollector.record expects a DiagnosticEntry"
            raise TypeError(msg)
        self._entries.append(entry)


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
    SUPPORTS_DIAGNOSTICS: ClassVar[bool] = False

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
