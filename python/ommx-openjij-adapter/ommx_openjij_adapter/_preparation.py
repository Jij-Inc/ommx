"""OpenJij-specific preparation domain values and diagnostics."""

from __future__ import annotations

from collections.abc import Iterator, Mapping
from dataclasses import dataclass
from math import isfinite
from types import MappingProxyType

from ommx import InstanceClassMembershipReport
from ommx.adapter import (
    AdapterApplicabilityReport,
    PreparationError,
    PreparationFailure,
    PreparationPolicy,
    PreparationTransform,
)

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
class OpenJijPreparationPolicy(PreparationPolicy):
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
        PreparationPolicy.__post_init__(self)
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
class OpenJijPreparationSourceCheck:
    """Structural membership evidence for a preparation source."""

    source_membership: InstanceClassMembershipReport

    @property
    def conditions_hold(self) -> bool:
        return self.source_membership.is_member


@dataclass(frozen=True, slots=True)
class OpenJijPreparationReport:
    """The Policy used and four outcomes of one preparation attempt.

    ``policy`` is the immutable settings audit. The outcome fields separately
    record the source check, applied transforms, materialization failures, and
    produced-input applicability.
    """

    policy: OpenJijPreparationPolicy
    source_check: OpenJijPreparationSourceCheck
    transforms: tuple[PreparationTransform, ...]
    preparation_failures: tuple[PreparationFailure, ...] = ()
    input_applicability: AdapterApplicabilityReport | None = None

    def __post_init__(self) -> None:
        if not self.source_check.conditions_hold:
            if (
                self.transforms
                or self.preparation_failures
                or self.input_applicability is not None
            ):
                raise ValueError(
                    "a rejected preparation source cannot have phase or input outcomes"
                )
            return
        if self.preparation_failures and self.input_applicability is not None:
            raise ValueError(
                "a preparation report cannot contain both phase failures and an "
                "input applicability result"
            )
        if not self.preparation_failures and self.input_applicability is None:
            raise ValueError(
                "an accepted preparation source requires either phase failures or "
                "an input applicability result"
            )

    @property
    def is_successful(self) -> bool:
        return (
            self.source_check.conditions_hold
            and not self.preparation_failures
            and self.input_applicability is not None
            and self.input_applicability.is_applicable
        )


class OpenJijPreparationError(PreparationError[OpenJijPreparationReport]):
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
        elif report.preparation_failures:
            details = "\n".join(
                f"- {failure.name}/{failure.reason}: {failure.description}"
                for failure in report.preparation_failures
            )
            message = f"OpenJij preparation failed:\n{details}"
        else:
            message = "OpenJij preparation did not produce an applicable input"
        ValueError.__init__(self, message)
