"""OpenJij preparation orchestration and public outcome projection."""

from __future__ import annotations

import copy
from collections.abc import Callable

from ommx import Instance, Kind, SpecialConstraintKind
from ommx.adapter import (
    AdapterApplicabilityReport,
    Preparation,
    PreparationPolicy,
    PreparationTransform,
)

from ._preparation import (
    OpenJijPreparationPolicy,
    OpenJijPreparationError,
    OpenJijPreparationReport,
    OpenJijPreparationSourceCheck,
)
from ._preparation_checks import check_preparation_source
from ._preparation_phases import (
    apply_penalties,
    encode_remaining_integers,
    encode_source_integers,
    lower_special_constraints,
    normalize_sense,
    prepare_inequalities,
)
from ._preparation_stages import (
    _Blocked,
    _CheckedAdapterInput,
    _InputRejected,
    _PhaseRejected,
    _PreparedInput,
    _PreparationAttempt,
    _ProvenInfeasible,
    _SourceMember,
    _SourceRejected,
)


def check_preparation(
    ommx_instance: Instance,
    *,
    check_input_applicability: Callable[[Instance], AdapterApplicabilityReport],
    candidate_special_constraint_lowerings: tuple[SpecialConstraintKind, ...],
    policy: PreparationPolicy | None = None,
) -> OpenJijPreparationReport:
    """Run explicit preparation on an isolated copy and return its report."""
    normalized_policy = _normalize_preparation_policy(policy)
    attempt = _run_preparation(
        ommx_instance,
        check_input_applicability=check_input_applicability,
        candidate_special_constraint_lowerings=candidate_special_constraint_lowerings,
        policy=normalized_policy,
    )
    return _report_for_attempt(normalized_policy, attempt)


def prepare(
    ommx_instance: Instance,
    *,
    check_input_applicability: Callable[[Instance], AdapterApplicabilityReport],
    candidate_special_constraint_lowerings: tuple[SpecialConstraintKind, ...],
    policy: PreparationPolicy | None = None,
) -> Preparation[OpenJijPreparationReport]:
    """Produce a separate Adapter input and an auditable preparation report."""
    normalized_policy = _normalize_preparation_policy(policy)
    attempt = _run_preparation(
        ommx_instance,
        check_input_applicability=check_input_applicability,
        candidate_special_constraint_lowerings=candidate_special_constraint_lowerings,
        policy=normalized_policy,
    )
    report = _report_for_attempt(normalized_policy, attempt)
    if not isinstance(attempt, _PreparedInput):
        raise OpenJijPreparationError(report)
    return Preparation._create(
        source=attempt.source_instance,
        input=attempt.take_input(),
        report=report,
        preserves_optimality=False,
    )


def _normalize_preparation_policy(
    policy: PreparationPolicy | None,
) -> OpenJijPreparationPolicy:
    if policy is None:
        return OpenJijPreparationPolicy()
    if isinstance(policy, OpenJijPreparationPolicy):
        return policy
    if isinstance(policy, PreparationPolicy):
        return OpenJijPreparationPolicy(
            allowed_special_constraint_lowerings=(
                policy.allowed_special_constraint_lowerings
            )
        )
    raise TypeError("policy must be a PreparationPolicy")


def _phase_rejected(
    source_check: OpenJijPreparationSourceCheck,
    completed_transforms: tuple[PreparationTransform, ...],
    outcome: _Blocked,
) -> _PhaseRejected:
    return _PhaseRejected(
        source_check=source_check,
        transforms=completed_transforms + outcome.transforms,
        failures=outcome.failures,
    )


def _run_preparation(
    ommx_instance: Instance,
    *,
    check_input_applicability: Callable[[Instance], AdapterApplicabilityReport],
    candidate_special_constraint_lowerings: tuple[SpecialConstraintKind, ...],
    policy: OpenJijPreparationPolicy,
) -> _PreparationAttempt:
    source_check = check_preparation_source(ommx_instance)
    if not source_check.conditions_hold:
        return _SourceRejected(source_check)

    source_instance = copy.deepcopy(ommx_instance)
    working = copy.deepcopy(ommx_instance)
    transforms: tuple[PreparationTransform, ...] = ()

    direct_applicability = check_input_applicability(working)
    if direct_applicability.is_applicable:
        return _PreparedInput(
            source_check=source_check,
            transforms=(),
            checked_input=_CheckedAdapterInput(working, direct_applicability),
            source_instance=source_instance,
        )

    declared_special_constraint_lowerings = frozenset(
        candidate_special_constraint_lowerings
    )
    allowed_special_constraint_lowerings = declared_special_constraint_lowerings
    if policy.allowed_special_constraint_lowerings is not None:
        allowed_special_constraint_lowerings &= (
            policy.allowed_special_constraint_lowerings
        )
    lowering = lower_special_constraints(
        _SourceMember(working, source_check),
        allowed_special_constraint_lowerings=allowed_special_constraint_lowerings,
    )
    if isinstance(lowering, _Blocked):
        return _phase_rejected(source_check, transforms, lowering)
    if isinstance(lowering, _ProvenInfeasible):
        return lowering
    regular_source = lowering.value
    transforms += lowering.transforms

    source_integer_ids = frozenset(
        variable.id
        for variable in source_instance.used_decision_variables
        if variable.kind == Kind.Integer
    )
    source_encoding = encode_source_integers(regular_source, source_integer_ids)
    if isinstance(source_encoding, _Blocked):
        return _phase_rejected(source_check, transforms, source_encoding)
    if isinstance(source_encoding, _ProvenInfeasible):
        return source_encoding
    source_encoded = source_encoding.value
    transforms += source_encoding.transforms

    normalization = normalize_sense(source_encoded)
    normalized_source = normalization.value
    transforms += normalization.transforms

    slack = prepare_inequalities(normalized_source, policy)
    if isinstance(slack, _Blocked):
        return _phase_rejected(source_check, transforms, slack)
    if isinstance(slack, _ProvenInfeasible):
        return slack
    penalty_ready = slack.value
    transforms += slack.transforms

    penalty = apply_penalties(penalty_ready, source_instance, policy)
    if isinstance(penalty, _Blocked):
        return _phase_rejected(source_check, transforms, penalty)
    if isinstance(penalty, _ProvenInfeasible):
        return penalty
    encoding_input = penalty.value
    transforms += penalty.transforms

    encoding = encode_remaining_integers(encoding_input)
    if isinstance(encoding, _Blocked):
        return _phase_rejected(source_check, transforms, encoding)
    if isinstance(encoding, _ProvenInfeasible):
        return encoding
    candidate = encoding.value
    transforms += encoding.transforms

    checked_input = _CheckedAdapterInput.check(candidate, check_input_applicability)
    if not checked_input.applicability.is_applicable:
        return _InputRejected(source_check, transforms, checked_input)
    return _PreparedInput(
        source_check=source_check,
        transforms=transforms,
        checked_input=checked_input,
        source_instance=source_instance,
    )


def _report_for_attempt(
    policy: OpenJijPreparationPolicy,
    attempt: _PreparationAttempt,
) -> OpenJijPreparationReport:
    if isinstance(attempt, _ProvenInfeasible):
        raise attempt.error
    if isinstance(attempt, _SourceRejected):
        return OpenJijPreparationReport(
            policy=policy,
            source_check=attempt.source_check,
            transforms=(),
        )
    if isinstance(attempt, _PhaseRejected):
        return OpenJijPreparationReport(
            policy=policy,
            source_check=attempt.source_check,
            transforms=attempt.transforms,
            preparation_failures=attempt.failures,
        )
    if isinstance(attempt, _InputRejected):
        return OpenJijPreparationReport(
            policy=policy,
            source_check=attempt.source_check,
            transforms=attempt.transforms,
            input_applicability=attempt.input_applicability,
        )
    return OpenJijPreparationReport(
        policy=policy,
        source_check=attempt.source_check,
        transforms=attempt.transforms,
        input_applicability=attempt.input_applicability,
    )
