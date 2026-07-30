"""Private stage values and outcomes for OpenJij preparation."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass, field
from math import isfinite
from typing import Generic, TypeAlias, TypeVar

from ommx import Equality, InfeasibleDetected, Instance, Kind, Sense
from ommx.adapter import AdapterApplicabilityReport

from ._preparation import (
    OpenJijPreparationFailure,
    OpenJijPreparationSourceCheck,
    OpenJijPreparationStep,
)


class _StageInvariantError(RuntimeError):
    """Raised when an internal preparation phase violates its postcondition."""


def _require(condition: bool, description: str) -> None:
    if not condition:
        raise _StageInvariantError(description)


def _require_no_special_constraints(instance: Instance) -> None:
    _require(
        not instance.indicator_constraints
        and not instance.one_hot_constraints
        and not instance.sos1_constraints,
        "preparation stage requires no active special constraints",
    )


def _require_regular_variable_kinds(instance: Instance) -> None:
    unsupported = {
        variable.id
        for variable in instance.used_decision_variables
        if variable.kind not in {Kind.Binary, Kind.Integer}
    }
    _require(
        not unsupported,
        f"preparation stage contains unsupported variable IDs: {sorted(unsupported)}",
    )


def _require_no_active_constraints(instance: Instance) -> None:
    _require_no_special_constraints(instance)
    _require(
        not instance.constraints,
        "preparation stage requires no active regular constraints",
    )


def _require_source_encoded(
    instance: Instance,
    source_integer_ids: frozenset[int],
) -> None:
    _require_no_special_constraints(instance)
    used_ids = {variable.id for variable in instance.used_decision_variables}
    _require(
        source_integer_ids.isdisjoint(used_ids),
        "source Integer IDs must not remain used after log encoding",
    )
    _require(
        all(
            variable.kind == Kind.Binary
            for variable in instance.used_decision_variables
        ),
        "only Binary variables may remain used after source Integer encoding",
    )


@dataclass(slots=True)
class _OwnedStage:
    """A single-use ownership token for the mutable Instance in one phase."""

    _instance: Instance = field(repr=False)
    _consumed: bool = field(default=False, init=False, repr=False, compare=False)

    @property
    def instance(self) -> Instance:
        _require(not self._consumed, "preparation stage has already been consumed")
        return self._instance

    def take_instance(self) -> Instance:
        instance = self.instance
        self._consumed = True
        return instance


@dataclass(slots=True)
class _SourceMember(_OwnedStage):
    source_check: OpenJijPreparationSourceCheck

    def __post_init__(self) -> None:
        from ._preparation_checks import check_preparation_source

        _require(
            self.source_check.conditions_hold,
            "source-member stage requires a successful source check",
        )
        _require(
            self.source_check == check_preparation_source(self.instance),
            "source membership evidence must describe the owned Instance",
        )


@dataclass(slots=True)
class _RegularSource(_OwnedStage):
    def __post_init__(self) -> None:
        _require_no_special_constraints(self.instance)
        _require_regular_variable_kinds(self.instance)


@dataclass(slots=True)
class _SourceEncoded(_OwnedStage):
    source_integer_ids: frozenset[int]

    def __post_init__(self) -> None:
        _require_source_encoded(self.instance, self.source_integer_ids)


@dataclass(slots=True)
class _MinimizationSource(_OwnedStage):
    source_integer_ids: frozenset[int]

    def __post_init__(self) -> None:
        _require_source_encoded(self.instance, self.source_integer_ids)
        _require(
            self.instance.sense == Sense.Minimize,
            "normalized source must be a minimization problem",
        )


@dataclass(frozen=True, slots=True)
class _ExactIntegerSlack:
    constraint_id: int


@dataclass(frozen=True, slots=True)
class _TrivialInequality:
    constraint_id: int


@dataclass(frozen=True, slots=True)
class _ApproximateIntegerSlack:
    constraint_id: int
    residual_step: float

    def __post_init__(self) -> None:
        _require(
            isfinite(self.residual_step) and self.residual_step > 0,
            "approximate slack residual step must be finite and positive",
        )


_SlackOutcome: TypeAlias = (
    _ExactIntegerSlack | _TrivialInequality | _ApproximateIntegerSlack
)


@dataclass(slots=True)
class _PenaltyReady(_OwnedStage):
    inequality_ids: frozenset[int]
    slack_outcomes: tuple[_SlackOutcome, ...]

    def __post_init__(self) -> None:
        _require_no_special_constraints(self.instance)
        _require_regular_variable_kinds(self.instance)
        _require(
            self.instance.sense == Sense.Minimize,
            "penalty-ready stage must be a minimization problem",
        )

        outcome_ids = [outcome.constraint_id for outcome in self.slack_outcomes]
        _require(
            len(outcome_ids) == len(set(outcome_ids)),
            "each inequality must have exactly one slack outcome",
        )
        _require(
            frozenset(outcome_ids) == self.inequality_ids,
            "slack outcomes must cover exactly the inequalities entering the phase",
        )

        for outcome in self.slack_outcomes:
            constraint = self.instance.constraints.get(outcome.constraint_id)
            if isinstance(outcome, _ExactIntegerSlack):
                _require(
                    constraint is not None
                    and constraint.equality == Equality.EqualToZero,
                    "exact slack outcome requires an active equality",
                )
            elif isinstance(outcome, _TrivialInequality):
                _require(
                    constraint is None
                    and outcome.constraint_id in self.instance.removed_constraints,
                    "trivial inequality outcome requires a removed constraint",
                )
            else:
                _require(
                    constraint is not None
                    and constraint.equality == Equality.LessThanOrEqualToZero,
                    "approximate slack outcome requires an active inequality",
                )

        active_inequality_ids = frozenset(
            constraint_id
            for constraint_id, constraint in self.instance.constraints.items()
            if constraint.equality == Equality.LessThanOrEqualToZero
        )
        approximate_ids = frozenset(
            outcome.constraint_id
            for outcome in self.slack_outcomes
            if isinstance(outcome, _ApproximateIntegerSlack)
        )
        _require(
            active_inequality_ids == approximate_ids,
            "only explicitly approximated inequalities may remain active",
        )


@dataclass(slots=True)
class _EncodingInput(_OwnedStage):
    def __post_init__(self) -> None:
        _require_no_active_constraints(self.instance)
        _require_regular_variable_kinds(self.instance)
        _require(
            self.instance.sense == Sense.Minimize,
            "Integer encoding input must be a minimization problem",
        )


@dataclass(slots=True)
class _AdapterInputCandidate(_OwnedStage):
    def __post_init__(self) -> None:
        _require_no_active_constraints(self.instance)
        _require(
            all(
                variable.kind == Kind.Binary
                for variable in self.instance.used_decision_variables
            ),
            "OpenJij input candidate must contain only Binary variables",
        )
        _require(
            self.instance.sense == Sense.Minimize,
            "OpenJij input candidate must be a minimization problem",
        )


@dataclass(slots=True)
class _CheckedAdapterInput:
    """Single-use ownership of a candidate and evidence computed from it."""

    _instance: Instance = field(repr=False)
    applicability: AdapterApplicabilityReport
    _consumed: bool = field(default=False, init=False, repr=False, compare=False)

    @classmethod
    def check(
        cls,
        candidate: _AdapterInputCandidate,
        checker: Callable[[Instance], AdapterApplicabilityReport],
    ) -> _CheckedAdapterInput:
        instance = candidate.take_instance()
        return cls(instance, checker(instance))

    def take_instance(self) -> Instance:
        _require(not self._consumed, "checked Adapter input has already been consumed")
        self._consumed = True
        return self._instance


_T = TypeVar("_T")


@dataclass(frozen=True, slots=True)
class _Applied(Generic[_T]):
    value: _T
    steps: tuple[OpenJijPreparationStep, ...] = ()


@dataclass(frozen=True, slots=True)
class _Blocked:
    failures: tuple[OpenJijPreparationFailure, ...]
    steps: tuple[OpenJijPreparationStep, ...] = ()

    def __post_init__(self) -> None:
        _require(bool(self.failures), "blocked phase requires at least one failure")


@dataclass(frozen=True, slots=True)
class _ProvenInfeasible:
    error: InfeasibleDetected


_PhaseOutcome: TypeAlias = _Applied[_T] | _Blocked | _ProvenInfeasible


@dataclass(frozen=True, slots=True)
class _SourceRejected:
    source_check: OpenJijPreparationSourceCheck

    def __post_init__(self) -> None:
        _require(
            not self.source_check.conditions_hold,
            "source rejection requires a failed source check",
        )


@dataclass(frozen=True, slots=True)
class _PhaseRejected:
    source_check: OpenJijPreparationSourceCheck
    steps: tuple[OpenJijPreparationStep, ...]
    failures: tuple[OpenJijPreparationFailure, ...]

    def __post_init__(self) -> None:
        _require(
            self.source_check.conditions_hold,
            "phase rejection requires an accepted source",
        )
        _require(bool(self.failures), "phase rejection requires failures")


@dataclass(frozen=True, slots=True)
class _InputRejected:
    source_check: OpenJijPreparationSourceCheck
    steps: tuple[OpenJijPreparationStep, ...]
    checked_input: _CheckedAdapterInput

    @property
    def input_applicability(self) -> AdapterApplicabilityReport:
        return self.checked_input.applicability

    def __post_init__(self) -> None:
        _require(
            self.source_check.conditions_hold,
            "input rejection requires an accepted source",
        )
        _require(
            not self.input_applicability.is_applicable,
            "input rejection requires a non-applicable candidate",
        )


@dataclass(frozen=True, slots=True)
class _PreparedInput:
    source_check: OpenJijPreparationSourceCheck
    steps: tuple[OpenJijPreparationStep, ...]
    checked_input: _CheckedAdapterInput
    source_instance: Instance = field(repr=False)

    @property
    def input_applicability(self) -> AdapterApplicabilityReport:
        return self.checked_input.applicability

    def take_input(self) -> Instance:
        return self.checked_input.take_instance()

    def __post_init__(self) -> None:
        _require(
            self.source_check.conditions_hold,
            "prepared input requires an accepted source",
        )
        _require(
            self.input_applicability.is_applicable,
            "prepared input requires an applicable candidate",
        )


_PreparationAttempt: TypeAlias = (
    _SourceRejected
    | _PhaseRejected
    | _InputRejected
    | _PreparedInput
    | _ProvenInfeasible
)
