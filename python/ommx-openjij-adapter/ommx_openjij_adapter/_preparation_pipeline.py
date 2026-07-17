"""OpenJij preparation orchestration and public outcome projection."""

from __future__ import annotations

import copy
from collections.abc import Callable

from ommx import Instance, Kind
from ommx.adapter import AdapterApplicabilityReport

from ._preparation import (
    OpenJijPreparation,
    OpenJijPreparationConfig,
    OpenJijPreparationError,
    OpenJijPreparationReport,
    OpenJijPreparationSourceCheck,
    OpenJijPreparationStep,
    _create_preparation,
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
    config: OpenJijPreparationConfig | None = None,
) -> OpenJijPreparationReport:
    """Run explicit preparation on an isolated copy and return its report."""
    normalized_config = _normalize_preparation_config(config)
    attempt = _run_preparation(
        ommx_instance,
        check_input_applicability=check_input_applicability,
        config=normalized_config,
    )
    return _report_for_attempt(normalized_config, attempt)


def prepare(
    ommx_instance: Instance,
    *,
    check_input_applicability: Callable[[Instance], AdapterApplicabilityReport],
    config: OpenJijPreparationConfig | None = None,
) -> OpenJijPreparation:
    """Produce a separate Adapter input and an auditable preparation report."""
    normalized_config = _normalize_preparation_config(config)
    attempt = _run_preparation(
        ommx_instance,
        check_input_applicability=check_input_applicability,
        config=normalized_config,
    )
    report = _report_for_attempt(normalized_config, attempt)
    if not isinstance(attempt, _PreparedInput):
        raise OpenJijPreparationError(report)
    return _create_preparation(
        input=attempt.take_input(),
        source_instance=attempt.source_instance,
        report=report,
    )


def _normalize_preparation_config(
    config: OpenJijPreparationConfig | None,
) -> OpenJijPreparationConfig:
    if config is None:
        return OpenJijPreparationConfig()
    if not isinstance(config, OpenJijPreparationConfig):
        raise TypeError("config must be an OpenJijPreparationConfig")
    return config


def _phase_rejected(
    source_check: OpenJijPreparationSourceCheck,
    completed_steps: tuple[OpenJijPreparationStep, ...],
    outcome: _Blocked,
) -> _PhaseRejected:
    return _PhaseRejected(
        source_check=source_check,
        steps=completed_steps + outcome.steps,
        failures=outcome.failures,
    )


def _run_preparation(
    ommx_instance: Instance,
    *,
    check_input_applicability: Callable[[Instance], AdapterApplicabilityReport],
    config: OpenJijPreparationConfig,
) -> _PreparationAttempt:
    source_check = check_preparation_source(ommx_instance)
    if not source_check.conditions_hold:
        return _SourceRejected(source_check)

    source_instance = copy.deepcopy(ommx_instance)
    working = copy.deepcopy(ommx_instance)
    steps: tuple[OpenJijPreparationStep, ...] = ()

    lowering = lower_special_constraints(_SourceMember(working, source_check))
    if isinstance(lowering, _Blocked):
        return _phase_rejected(source_check, steps, lowering)
    if isinstance(lowering, _ProvenInfeasible):
        return lowering
    regular_source = lowering.value
    steps += lowering.steps

    source_integer_ids = frozenset(
        variable.id
        for variable in source_instance.used_decision_variables
        if variable.kind == Kind.Integer
    )
    source_encoding = encode_source_integers(regular_source, source_integer_ids)
    if isinstance(source_encoding, _Blocked):
        return _phase_rejected(source_check, steps, source_encoding)
    if isinstance(source_encoding, _ProvenInfeasible):
        return source_encoding
    source_encoded = source_encoding.value
    steps += source_encoding.steps

    normalization = normalize_sense(source_encoded)
    normalized_source = normalization.value
    steps += normalization.steps

    slack = prepare_inequalities(normalized_source, config)
    if isinstance(slack, _Blocked):
        return _phase_rejected(source_check, steps, slack)
    if isinstance(slack, _ProvenInfeasible):
        return slack
    penalty_ready = slack.value
    steps += slack.steps

    penalty = apply_penalties(penalty_ready, source_instance, config)
    if isinstance(penalty, _Blocked):
        return _phase_rejected(source_check, steps, penalty)
    if isinstance(penalty, _ProvenInfeasible):
        return penalty
    encoding_input = penalty.value
    steps += penalty.steps

    encoding = encode_remaining_integers(encoding_input)
    if isinstance(encoding, _Blocked):
        return _phase_rejected(source_check, steps, encoding)
    if isinstance(encoding, _ProvenInfeasible):
        return encoding
    candidate = encoding.value
    steps += encoding.steps

    checked_input = _CheckedAdapterInput.check(candidate, check_input_applicability)
    if not checked_input.applicability.is_applicable:
        return _InputRejected(source_check, steps, checked_input)
    return _PreparedInput(
        source_check=source_check,
        steps=steps,
        checked_input=checked_input,
        source_instance=source_instance,
    )


def _report_for_attempt(
    config: OpenJijPreparationConfig,
    attempt: _PreparationAttempt,
) -> OpenJijPreparationReport:
    if isinstance(attempt, _ProvenInfeasible):
        raise attempt.error
    if isinstance(attempt, _SourceRejected):
        return OpenJijPreparationReport(
            config=config,
            source_check=attempt.source_check,
            steps=(),
        )
    if isinstance(attempt, _PhaseRejected):
        return OpenJijPreparationReport(
            config=config,
            source_check=attempt.source_check,
            steps=attempt.steps,
            preparation_failures=attempt.failures,
        )
    if isinstance(attempt, _InputRejected):
        return OpenJijPreparationReport(
            config=config,
            source_check=attempt.source_check,
            steps=attempt.steps,
            input_applicability=attempt.input_applicability,
        )
    return OpenJijPreparationReport(
        config=config,
        source_check=attempt.source_check,
        steps=attempt.steps,
        input_applicability=attempt.input_applicability,
    )
