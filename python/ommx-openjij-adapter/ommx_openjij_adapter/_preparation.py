"""OpenJij-specific preparation results and diagnostics."""

from __future__ import annotations

import copy
from dataclasses import dataclass, field

from ommx import Instance, InstanceClassMembershipReport, Samples, SampleSet, State
from ommx.adapter import (
    AdapterApplicabilityReport,
    AdapterPreconditionViolation,
    ConstraintRef,
)

PreparationDiagnosticValue = str | int | float | bool | None


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
    """Checks, operation audit, failures, and produced-input applicability."""

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
