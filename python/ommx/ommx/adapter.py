from __future__ import annotations

import json
import math
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from typing import Any, ClassVar, Mapping, Protocol, TypeAlias, TypeVar, cast

from ommx.v1 import Instance, Solution, SampleSet, AdditionalCapability


SolverInput = Any
SolverOutput = Any
SamplerInput = Any
SamplerOutput = Any
JsonScalar: TypeAlias = str | int | float | bool | None
JsonValue: TypeAlias = JsonScalar | list["JsonValue"] | dict[str, "JsonValue"]
JsonObject: TypeAlias = dict[str, JsonValue]

DIAGNOSTIC_SCHEMA_ANNOTATION = "org.ommx.diagnostic.schema"
DIAGNOSTIC_KIND_ANNOTATION = "org.ommx.diagnostic.kind"

D = TypeVar("D", bound="JsonDiagnostic")


class JsonDiagnostic(Protocol):
    """Adapter-defined diagnostic data that can be serialized as JSON."""

    SCHEMA: ClassVar[str]
    NAME: ClassVar[str]
    KIND: ClassVar[str]

    def to_json(self) -> JsonObject:
        """Return the JSON object representation for persistence."""
        ...

    @classmethod
    def from_json(cls, data: JsonObject) -> JsonDiagnostic:
        """Reconstruct this diagnostic from its JSON object representation."""
        ...

    def to_entry(self) -> DiagnosticEntry:
        """Serialize this diagnostic to the storage boundary representation."""
        ...


@dataclass(slots=True)
class DiagnosticEntry:
    """Serialized diagnostic payload at the storage boundary.

    Users should usually interact with adapter-defined ``JsonDiagnostic``
    types such as SCIP termination reports. ``DiagnosticEntry`` is the bytes
    representation used when diagnostics are persisted or transported.
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

    @property
    def schema(self) -> str | None:
        return self.annotations.get(DIAGNOSTIC_SCHEMA_ANNOTATION)

    @property
    def kind(self) -> str | None:
        return self.annotations.get(DIAGNOSTIC_KIND_ANNOTATION)

    @classmethod
    def from_json_diagnostic(
        cls,
        diagnostic: JsonDiagnostic,
        *,
        annotations: Mapping[str, str] | None = None,
    ) -> DiagnosticEntry:
        """Serialize an adapter-defined JSON diagnostic to an entry."""

        merged_annotations = {
            DIAGNOSTIC_SCHEMA_ANNOTATION: diagnostic.SCHEMA,
            DIAGNOSTIC_KIND_ANNOTATION: diagnostic.KIND,
        }
        for key, value in (annotations or {}).items():
            if key in merged_annotations and merged_annotations[key] != value:
                msg = (
                    f"Diagnostic annotation `{key}` conflicts with the diagnostic type"
                )
                raise ValueError(msg)
            merged_annotations[key] = value

        return cls(
            diagnostic.NAME,
            "application/json",
            _stable_json_bytes(diagnostic.to_json()),
            merged_annotations,
        )

    def decode_as(self, diagnostic_type: type[D]) -> D:
        """Deserialize this entry as the caller-provided diagnostic type."""

        if self.media_type != "application/json":
            msg = f"Diagnostic entry media type is {self.media_type}, expected application/json"
            raise ValueError(msg)
        if self.schema != diagnostic_type.SCHEMA:
            msg = (
                f"Diagnostic entry schema is {self.schema!r}, "
                f"expected {diagnostic_type.SCHEMA!r}"
            )
            raise ValueError(msg)
        data = json.loads(self.data)
        if not isinstance(data, dict):
            msg = "JSON diagnostic payload must decode to an object"
            raise ValueError(msg)
        _validate_json_object(data)
        return cast(D, diagnostic_type.from_json(data))


class DiagnosticsSink(Protocol):
    """Protocol consumed by adapters that can emit diagnostics."""

    def record(self, diagnostic: JsonDiagnostic) -> None:
        """Record one diagnostic entry."""


class DiagnosticCollector:
    """In-memory diagnostics sink for direct adapter calls."""

    def __init__(self) -> None:
        self._diagnostics: list[JsonDiagnostic] = []

    @property
    def diagnostics(self) -> tuple[JsonDiagnostic, ...]:
        return tuple(self._diagnostics)

    @property
    def entries(self) -> tuple[DiagnosticEntry, ...]:
        return tuple(diagnostic.to_entry() for diagnostic in self._diagnostics)

    def record(self, diagnostic: JsonDiagnostic) -> None:
        _validate_json_diagnostic(diagnostic)
        self._diagnostics.append(diagnostic)


def _stable_json_bytes(value: JsonObject) -> bytes:
    _validate_json_object(value)
    return json.dumps(
        value,
        allow_nan=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode()


def _validate_json_diagnostic(diagnostic: object) -> None:
    diagnostic_type = type(diagnostic)
    for attr in ("SCHEMA", "NAME", "KIND"):
        value = getattr(diagnostic_type, attr, None)
        if not isinstance(value, str) or not value:
            msg = f"JsonDiagnostic {attr} must be a non-empty class string"
            raise TypeError(msg)
    for method in ("to_json", "to_entry"):
        if not callable(getattr(diagnostic, method, None)):
            msg = f"JsonDiagnostic must define {method}()"
            raise TypeError(msg)
    from_json = getattr(diagnostic_type, "from_json", None)
    if not callable(from_json):
        msg = "JsonDiagnostic must define from_json()"
        raise TypeError(msg)


def _validate_json_object(value: object) -> None:
    if not isinstance(value, dict):
        msg = "JSON diagnostic payload must be a dict"
        raise TypeError(msg)
    for key, item in value.items():
        if not isinstance(key, str):
            msg = "JSON diagnostic object keys must be strings"
            raise TypeError(msg)
        _validate_json_value(item)


def _validate_json_value(value: object) -> None:
    if value is None or isinstance(value, str | bool | int):
        return
    if isinstance(value, float):
        if not math.isfinite(value):
            msg = "JSON diagnostic floats must be finite"
            raise ValueError(msg)
        return
    if isinstance(value, list):
        for item in value:
            _validate_json_value(item)
        return
    if isinstance(value, dict):
        _validate_json_object(value)
        return
    msg = f"Object of type {type(value).__name__} is not JSON diagnostic data"
    raise TypeError(msg)


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
