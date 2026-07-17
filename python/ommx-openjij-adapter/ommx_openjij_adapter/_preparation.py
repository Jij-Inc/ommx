"""OpenJij-specific preparation domain values and diagnostics."""

from __future__ import annotations

import copy
from collections.abc import Iterator, Mapping
from dataclasses import dataclass, field
from math import isfinite
from types import MappingProxyType

from ommx import Instance, InstanceClassMembershipReport, Samples, SampleSet, State
from ommx.adapter import (
    AdapterApplicabilityReport,
    AdapterPreconditionViolation,
    ConstraintRef,
)

PreparationDiagnosticValue = str | int | float | bool | None
_MAX_U64 = 2**64 - 1


class _ImmutablePenaltyWeights(Mapping[int, float]):
    """A read-only Mapping snapshot that remains safe to copy in reports."""

    __slots__ = ("_values",)

    def __init__(self, values: Mapping[int, float]) -> None:
        object.__setattr__(self, "_values", MappingProxyType(dict(values)))

    def __getitem__(self, key: int) -> float:
        return self._values[key]

    def __iter__(self) -> Iterator[int]:
        return iter(self._values)

    def __len__(self) -> int:
        return len(self._values)

    def __repr__(self) -> str:
        return repr(dict(self._values))

    def __copy__(self) -> _ImmutablePenaltyWeights:
        return self

    def __deepcopy__(self, _memo: dict[int, object]) -> _ImmutablePenaltyWeights:
        return self

    def __hash__(self) -> int:
        return hash(frozenset(self._values.items()))

    def __setattr__(self, name: str, value: object) -> None:
        raise AttributeError(f"{type(self).__name__} is immutable")

    def __delattr__(self, name: str) -> None:
        raise AttributeError(f"{type(self).__name__} is immutable")


def _is_positive_finite(value: float) -> bool:
    if isinstance(value, bool):
        return False
    try:
        return isfinite(value) and value > 0
    except (TypeError, OverflowError):
        return False


@dataclass(frozen=True, slots=True)
class OpenJijPreparationConfig:
    """User-selected settings for one OpenJij preparation operation.

    The two penalty modes are mutually exclusive. Every configured penalty
    weight must be finite and positive, every per-constraint key must be a
    valid unsigned 64-bit constraint ID, the integer slack range must fit a
    positive unsigned 64-bit integer, and the per-constraint mapping is
    snapshotted so that reports remain auditable after construction.
    """

    uniform_penalty_weight: float | None = None
    penalty_weights: Mapping[int, float] | None = None
    inequality_integer_slack_max_range: int = 32
    allow_approximate_integer_slack: bool = False

    def __post_init__(self) -> None:
        if self.uniform_penalty_weight is not None and self.penalty_weights is not None:
            raise ValueError(
                "uniform_penalty_weight and penalty_weights are mutually exclusive"
            )

        if self.uniform_penalty_weight is not None and not _is_positive_finite(
            self.uniform_penalty_weight
        ):
            raise ValueError("uniform_penalty_weight must be a finite positive number")

        if self.penalty_weights is not None:
            if not isinstance(self.penalty_weights, Mapping):
                raise TypeError("penalty_weights must be a mapping")
            snapshot = dict(self.penalty_weights)
            for constraint_id, weight in snapshot.items():
                if (
                    not isinstance(constraint_id, int)
                    or isinstance(constraint_id, bool)
                    or not 0 <= constraint_id <= _MAX_U64
                ):
                    raise ValueError(
                        "penalty_weights keys must be integer constraint IDs in "
                        f"[0, {_MAX_U64}]"
                    )
                if not _is_positive_finite(weight):
                    raise ValueError(
                        "penalty weight for constraint "
                        f"{constraint_id} must be a finite positive number"
                    )
            object.__setattr__(
                self,
                "penalty_weights",
                _ImmutablePenaltyWeights(snapshot),
            )

        slack_range = self.inequality_integer_slack_max_range
        if (
            not isinstance(slack_range, int)
            or isinstance(slack_range, bool)
            or not 0 < slack_range <= _MAX_U64
        ):
            raise ValueError(
                "inequality_integer_slack_max_range must be an integer in "
                f"[1, {_MAX_U64}]"
            )

        if not isinstance(self.allow_approximate_integer_slack, bool):
            raise TypeError("allow_approximate_integer_slack must be a bool")


@dataclass(frozen=True, slots=True)
class OpenJijPreparationStep:
    """One OpenJij-specific operation recorded for preparation auditing.

    This record is not a composed mathematical guarantee. The common guarantee
    and policy contracts are tracked separately in OMMX issue #1111.
    """

    operation: str
    description: str
    variable_ids: frozenset[int] = field(default_factory=frozenset)
    constraint_refs: frozenset[ConstraintRef] = field(default_factory=frozenset)


@dataclass(frozen=True, slots=True)
class OpenJijPreparationSourceCheck:
    """Membership and Adapter-owned preconditions for a preparation source."""

    source_membership: InstanceClassMembershipReport
    preconditions_checked: bool
    precondition_violations: tuple[AdapterPreconditionViolation, ...]

    def __post_init__(self) -> None:
        if self.preconditions_checked != self.source_membership.is_member:
            raise ValueError(
                "preconditions_checked must be true exactly when source membership holds"
            )
        if not self.preconditions_checked and self.precondition_violations:
            raise ValueError(
                "precondition violations require preparation preconditions to be checked"
            )

    @property
    def conditions_hold(self) -> bool:
        return (
            self.source_membership.is_member
            and self.preconditions_checked
            and not self.precondition_violations
        )


@dataclass(frozen=True, slots=True)
class OpenJijPreparationFailure:
    """One failure discovered while materializing an accepted source."""

    reason: str
    description: str
    variable_ids: frozenset[int] = field(default_factory=frozenset)
    constraint_refs: frozenset[ConstraintRef] = field(default_factory=frozenset)
    observed: PreparationDiagnosticValue = None
    expected: PreparationDiagnosticValue = None


@dataclass(frozen=True, slots=True)
class OpenJijPreparationReport:
    """The Config used and four outcomes of one preparation attempt.

    ``config`` is the immutable settings audit. The outcome fields separately
    record the source check, applied steps, materialization failures, and
    produced-input applicability.
    """

    config: OpenJijPreparationConfig
    source_check: OpenJijPreparationSourceCheck
    steps: tuple[OpenJijPreparationStep, ...]
    preparation_failures: tuple[OpenJijPreparationFailure, ...] = ()
    input_applicability: AdapterApplicabilityReport | None = None

    @property
    def is_successful(self) -> bool:
        return (
            self.source_check.conditions_hold
            and not self.preparation_failures
            and self.input_applicability is not None
            and self.input_applicability.is_applicable
        )


@dataclass(frozen=True, slots=True)
class OpenJijPreparation:
    """A separate Adapter input together with source-state reevaluation."""

    _input: Instance = field(repr=False)
    _source_instance: Instance = field(repr=False)
    report: OpenJijPreparationReport

    def __post_init__(self) -> None:
        if not self.report.is_successful:
            raise ValueError("OpenJijPreparation requires a successful report")

    @property
    def input(self) -> Instance:
        """Return an isolated copy of the Binary, unconstrained minimization input."""
        return copy.deepcopy(self._input)

    def evaluate_source(self, sample_set: SampleSet) -> SampleSet:
        """Reevaluate input-side sample states against the source Instance.

        ``sample_set`` must have been evaluated against this preparation's
        :attr:`input`, which populates irrelevant and dependent source variables.
        """
        source_variable_ids = {
            variable.id for variable in self._source_instance.used_decision_variables
        }
        source_samples = Samples({})
        for sample_id in sorted(sample_set.sample_ids()):
            prepared_state = sample_set.get(sample_id).state
            entries: list[tuple[int, float]] = []
            for variable_id in source_variable_ids:
                value = prepared_state.get(variable_id)
                if value is None:
                    raise RuntimeError(
                        "OpenJij preparation did not reconstruct source variable "
                        f"ID {variable_id}"
                    )
                entries.append((variable_id, value))
            source_samples.append([sample_id], State(entries=entries))
        return self._source_instance.evaluate_samples(source_samples)


class OpenJijPreparationError(ValueError):
    """Raised when explicit OpenJij preparation cannot produce an input."""

    report: OpenJijPreparationReport

    def __init__(self, report: OpenJijPreparationReport):
        self.report = report
        source_check = report.source_check
        if not source_check.source_membership.is_member:
            message = (
                "OpenJij preparation source is outside its supported source class:\n"
                f"{source_check.source_membership}"
            )
        elif source_check.precondition_violations:
            details = "\n".join(
                f"- {violation.condition}: {violation.description}"
                for violation in source_check.precondition_violations
            )
            message = f"OpenJij preparation preconditions failed:\n{details}"
        elif report.preparation_failures:
            details = "\n".join(
                f"- {failure.reason}: {failure.description}"
                for failure in report.preparation_failures
            )
            message = f"OpenJij preparation failed:\n{details}"
        else:
            message = "OpenJij preparation did not produce an applicable input"
        super().__init__(message)
